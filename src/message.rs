use std::ops::IndexMut;

use dioxus::prelude::Signal;
use dioxus::signals::Writable;

use crate::{CURRENT_STABLE, get_rows, PAGE, PAGE_SIZE, ROBOT_ID, TableData};

type UT = Signal<TableData>;

pub enum Message {
    ChangeStable(String, i64, UT),
    StableFilter(String, UT),
    PrevPage(UT),
    NextPage(UT),
    Resizing(i64, i64, i64, UT),
    ResizeOver(UT),
}

pub fn cal_widths(
    width: i64,
    size: i64,
    changed_widths: Vec<i64>,
    real_moving_widths: Vec<i64>,
) -> Vec<i64> {
    let mut widths: Vec<i64> = Vec::new();
    let each_width = width / size as i64;
    for _ in 0..size {
        widths.push(each_width);
    }

    // 最后一个宽度
    let last_column = widths.index_mut(size as usize - 1);
    *last_column = width - each_width * (size as i64 - 1);

    // 累计值
    for (index, item) in changed_widths.iter().enumerate() {
        let tmp = widths.index_mut(index);
        *tmp += item.clone() as i64;
    }

    // 变化
    for (index, item) in real_moving_widths.iter().enumerate() {
        let tmp = widths.index_mut(index);
        *tmp += item.clone() as i64;
    }

    widths
}

pub async fn message_handler(msg: Message) {
    match msg {
        Message::ChangeStable(stable, size, mut table_data_state) => {
            PAGE.lock().unwrap().set(1);
            CURRENT_STABLE.lock().unwrap().set(stable);
            let start = std::time::Instant::now();

            let (rows, total_size, headers) = get_rows().await;
            table_data_state.with_mut(|data| {
                let l = headers.len();
                let total_page: i64;
                if total_size / PAGE_SIZE == 0 && total_size > PAGE_SIZE {
                    total_page = total_size / PAGE_SIZE;
                } else {
                    total_page = total_size / PAGE_SIZE + 1;
                };
                (
                    data.headers,
                    data.rows,
                    data.total_size,
                    data.total_page,
                    data.changed_size,
                    data.real_moving_size,
                    data.widths,
                    data.spend,
                ) = (
                    headers,
                    rows,
                    total_size,
                    total_page,
                    vec![0; l],
                    vec![0; l],
                    cal_widths(size, l as i64, vec![0; l], vec![0; l]),
                    start.elapsed().as_millis().to_string(),
                )
            });
        }

        Message::StableFilter(search_robot_id, mut table_data_state) => {
            PAGE.lock().unwrap().set(1);
            ROBOT_ID.lock().unwrap().set(search_robot_id);
            let start = std::time::Instant::now();
            let (rows, total_size, headers) = get_rows().await;

            table_data_state.with_mut(|data| {
                let total_page: i64;
                if total_size / PAGE_SIZE == 0 && total_size > PAGE_SIZE {
                    total_page = total_size / PAGE_SIZE;
                } else {
                    total_page = total_size / PAGE_SIZE + 1;
                };
                (
                    data.headers,
                    data.rows,
                    data.total_page,
                    data.total_size,
                    data.spend,
                ) = (
                    headers,
                    rows,
                    total_page,
                    total_size,
                    start.elapsed().as_millis().to_string(),
                );
            });
        }
        Message::PrevPage(mut table_data_state) => {
            let mut page = PAGE.lock().unwrap().get();
            if page - 1 < 1 {
                page = 1;
            } else {
                page -= 1;
            }
            PAGE.lock().unwrap().set(page);
            let start = std::time::Instant::now();
            let (rows, total_size, headers) = get_rows().await;
            table_data_state.with_mut(|data| {
                let total_page: i64;
                if total_size / PAGE_SIZE == 0 && total_size > PAGE_SIZE {
                    total_page = total_size / PAGE_SIZE;
                } else {
                    total_page = total_size / PAGE_SIZE + 1;
                };
                (
                    data.headers,
                    data.rows,
                    data.total_page,
                    data.total_size,
                    data.spend,
                ) = (
                    headers,
                    rows,
                    total_page,
                    total_size,
                    start.elapsed().as_millis().to_string(),
                );
            });
        }
        Message::NextPage(mut table_data_state) => {
            let page = PAGE.lock().unwrap().get();
            PAGE.lock().unwrap().set(page + 1);
            let start = std::time::Instant::now();
            let (rows, total_size, headers) = get_rows().await;
            table_data_state.with_mut(|data| {
                let total_page: i64;
                if total_size / PAGE_SIZE == 0 && total_size > PAGE_SIZE {
                    total_page = total_size / PAGE_SIZE;
                } else {
                    total_page = total_size / PAGE_SIZE + 1;
                };
                (
                    data.headers,
                    data.rows,
                    data.total_page,
                    data.total_size,
                    data.spend,
                ) = (
                    headers,
                    rows,
                    total_page,
                    total_size,
                    start.elapsed().as_millis().to_string(),
                );
            });
        }
        Message::Resizing(width, index, size, mut table_data_state) => {
            table_data_state.with_mut(|data| {
                *data.real_moving_size.index_mut(index as usize) = size;
                data.widths = cal_widths(
                    width,
                    data.headers.len() as i64,
                    data.changed_size.clone(),
                    data.real_moving_size.clone(),
                );
            });
        }
        Message::ResizeOver(mut table_data_state) => {
            table_data_state.with_mut(|data| {
                for (index, value) in data.real_moving_size.iter().enumerate() {
                    *data.changed_size.index_mut(index) += value;
                }
                data.real_moving_size = vec![0; data.headers.len()];
            });
        }
    }
}
