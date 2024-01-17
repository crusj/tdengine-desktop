use std::ops::IndexMut;
use std::rc::Rc;

use dioxus::hooks::UseState;

use crate::{CURRENT_STABLE, get_rows, PAGE, ROBOT_ID, TableData};

pub enum Message {
    ChangeStable(String, i64, UseState<TableData>),
    StableFilter(String, UseState<TableData>),
    PrevPage(UseState<TableData>),
    NextPage(UseState<TableData>),
    Resizing(i64, i64, i64, UseState<TableData>),
    ResizeOver(UseState<TableData>),
}

pub fn cal_widths(width: i64, size: i64, changed_widths: Vec<i64>, real_moving_widths: Vec<i64>) -> Vec<i64> {
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

pub fn message_handler(runtime: Rc<tokio::runtime::Runtime>, msg: Message) {
    match msg {
        Message::ChangeStable(stable, size, table_data_state) => {
            PAGE.lock().unwrap().set(1);
            CURRENT_STABLE.lock().unwrap().set(stable);
            let start = std::time::Instant::now();
            let (rows, total_size, headers) = runtime.block_on(async { get_rows().await });
            let l = headers.len();
            let total_page: i64;
            if total_size / 20 == 0 && total_size > 20 {
                total_page = total_size / 20;
            } else {
                total_page = total_size / 20 + 1;
            };
            table_data_state.set(TableData {
                runtime: runtime.clone(),
                headers,
                rows,
                total_size,
                total_page,
                changed_size: vec![0; l],
                real_moving_size: vec![0; l],
                widths: cal_widths(size, l as i64, vec![0; l], vec![0; l]),
                spend: start.elapsed().as_millis().to_string(),
            });
        }

        Message::StableFilter(search_robot_id, table_data_state) => {
            if !search_robot_id.is_empty() {
                PAGE.lock().unwrap().set(1);
                ROBOT_ID.lock().unwrap().set(search_robot_id);
                let start = std::time::Instant::now();
                let (rows, total_size, headers) = runtime.block_on(async { get_rows().await });

                let table_data = table_data_state.get();
                let total_page: i64;
                if total_size / 20 == 0 && total_size > 20 {
                    total_page = total_size / 20;
                } else {
                    total_page = total_size / 20 + 1;
                };
                table_data_state.set(TableData {
                    runtime: runtime.clone(),
                    headers,
                    rows,
                    total_page,
                    total_size,
                    changed_size: table_data.changed_size.clone(),
                    real_moving_size: table_data.real_moving_size.clone(),
                    widths: table_data.widths.clone(),
                    spend: start.elapsed().as_millis().to_string(),
                });
            }
        }
        Message::PrevPage(table_data_state) => {
            let mut page = PAGE.lock().unwrap().get();
            if page - 1 < 1 {
                page = 1;
            } else {
                page -= 1;
            }
            PAGE.lock().unwrap().set(page);
            let start = std::time::Instant::now();
            let (rows, total_size, headers) = runtime.block_on(async { get_rows().await });

            let table_data = table_data_state.get();
            let total_page: i64;
            if total_size / 20 == 0 && total_size > 20 {
                total_page = total_size / 20;
            } else {
                total_page = total_size / 20 + 1;
            };
            table_data_state.set(TableData {
                runtime: runtime.clone(),
                headers,
                rows,
                total_size,
                total_page,
                changed_size: table_data.changed_size.clone(),
                real_moving_size: table_data.real_moving_size.clone(),
                widths: table_data.widths.clone(),
                spend: start.elapsed().as_millis().to_string(),
            });
        }
        Message::NextPage(table_data_state) => {
            let page = PAGE.lock().unwrap().get();
            PAGE.lock().unwrap().set(page + 1);
            let start = std::time::Instant::now();
            let (rows, total_size, headers) = runtime.block_on(async { get_rows().await });

            let table_data = table_data_state.get();
            let total_page: i64;
            if total_size / 20 == 0 && total_size > 20 {
                total_page = total_size / 20;
            } else {
                total_page = total_size / 20 + 1;
            };
            table_data_state.set(TableData {
                runtime: runtime.clone(),
                headers,
                rows,
                total_size,
                total_page,
                changed_size: table_data.changed_size.clone(),
                real_moving_size: table_data.real_moving_size.clone(),
                widths: table_data.widths.clone(),
                spend: start.elapsed().as_millis().to_string(),
            });
        }
        Message::Resizing(width, index, size, table_data_state) => {
            let table_data = table_data_state.get();
            let mut real_moving_size = table_data.real_moving_size.clone();
            *real_moving_size.index_mut(index as usize) = size;

            let headers = table_data.headers.clone();
            let l = headers.len();
            let new_widths = cal_widths(width, l as i64, table_data.changed_size.clone(), table_data.real_moving_size.clone());
            // println!("{} {} {} {:?} {}", width, index, size, new_widths, l);
            table_data_state.set(TableData {
                runtime: runtime.clone(),
                headers,
                rows: table_data.rows.clone(),
                total_page: table_data.total_page.clone(),
                total_size: table_data.total_size.clone(),
                changed_size: table_data.changed_size.clone(),
                real_moving_size,
                widths: new_widths,
                spend: table_data.spend.clone(),
            });
        }
        Message::ResizeOver(table_data_state) => {
            let table_data = table_data_state.get();
            let l = table_data.headers.len();
            let mut tmp = table_data.changed_size.clone();
            let tmp2 = table_data.real_moving_size.clone();
            for (index, value) in tmp2.iter().enumerate() {
                *tmp.index_mut(index) += value;
            }

            table_data_state.set(TableData {
                runtime: runtime.clone(),
                headers: table_data.headers.clone(),
                rows: table_data.rows.clone(),
                total_size: table_data.total_size.clone(),
                total_page: table_data.total_page,
                changed_size: tmp,
                real_moving_size: vec![0; l],
                widths: table_data.widths.clone(),
                spend: table_data.spend.clone(),
            });
        }
    }
}
