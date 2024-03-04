use std::cell::{Cell, OnceCell};
use std::io::prelude::*;
use std::ops::{Index, IndexMut};
use std::string::ToString;
use std::sync::Mutex;

use dioxus::desktop::{Config, WindowBuilder, wry};
use dioxus::prelude::*;
use dioxus_desktop::LogicalSize;
use dioxus_desktop::tao::dpi::Size::Logical;
use lazy_static::lazy_static;
use taos::*;

use config::CONF;
use message::*;

use crate::config::Source;

mod config;
mod log;
mod message;
mod td;

static RESIZING: Mutex<Cell<bool>> = Mutex::new(Cell::new(false));
static X: Mutex<Cell<f64>> = Mutex::new(Cell::new(0.0));
static TAOS: Mutex<OnceCell<Vec<HostData>>> = Mutex::new(OnceCell::new());
static TIMES: Mutex<Cell<i64>> = Mutex::new(Cell::new(0));
static PAGE_SIZE: i64 = 30;

// 这里写死了无法动态计算比例大小.
static SIZE: (i64, i64) = {
    let nav_size = (1800.0 * 0.25) as i64;
    (nav_size, 1800 - nav_size)
};

lazy_static! {
    static ref CURRENT_STABLE: Mutex<Cell<String>> = Mutex::new(Cell::new(String::default()));
    static ref PAGE: Mutex<Cell<i32>> = Mutex::new(Cell::new(1));
    static ref ROBOT_ID: Mutex<Cell<String>> = Mutex::new(Cell::new(String::default()));
}
static DATA: Mutex<OnceCell<TableData>> = Mutex::new(OnceCell::new());

fn main() {
    {
        let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
        runtime.block_on(async {
            let mut hosts: Vec<HostData> = Vec::new();
            for conf in CONF.sources.iter() {
                let host = connect_host(conf.clone()).await;
                hosts.push(host);
            }
            let current_stable = hosts
                .get(0)
                .unwrap()
                .stables
                .get(0)
                .clone()
                .unwrap()
                .clone();
            CURRENT_STABLE.lock().unwrap().set(current_stable);
            let _ = TAOS.lock().unwrap().set(hosts);

            let start = std::time::Instant::now();
            let (rows, total_size, headers) = get_rows().await;
            let l = headers.len();
            let changed_size = vec![0; headers.len()];
            let real_moving_size = vec![0; headers.len()];
            let total_page: i64;
            if total_size / PAGE_SIZE == 0 && total_size > PAGE_SIZE {
                total_page = total_size / PAGE_SIZE;
            } else {
                total_page = total_size / PAGE_SIZE + 1;
            };
            let table_data = TableData {
                headers,
                rows,
                total_size,
                total_page,
                changed_size: changed_size.clone(),
                real_moving_size: real_moving_size.clone(),
                widths: cal_widths(SIZE.1, l as i64, changed_size, real_moving_size),
                spend: start.elapsed().as_millis().to_string(),
            };

            // 不知道改如何将这个数据传入App,只能借助全局变量
            DATA.lock().unwrap().set(table_data).unwrap();
        });
    }

    LaunchBuilder::desktop().with_cfg(
        Config::new()
            .with_custom_head(r#"<link rel="stylesheet" href="public/tailwind.css">"#.to_string())
            .with_disable_context_menu(false)
            .with_window(WindowBuilder::new().with_resizable(true).with_inner_size(
                Logical(LogicalSize::new(1800.0, 1200.0))
            )),
    ).launch(App);
}

async fn get_rows() -> (Vec<Vec<String>>, i64, Vec<String>) {
    let page = PAGE.lock().unwrap().get();
    let stable = CURRENT_STABLE.lock().unwrap().get_mut().clone();
    let robot_id = ROBOT_ID.lock().unwrap().get_mut().clone();
    let robot_id = if robot_id.is_empty() {
        None
    } else {
        Some(robot_id)
    };

    let (mut rt, count) = {
        let mut taos = TAOS.lock().unwrap();
        let taos = taos.get_mut().unwrap();
        let taos = match &taos.index(0).taos {
            Some(taos) => taos,
            _ => {
                panic!("")
            }
        };
        let rows = td::STable::new(stable)
            .get_rows(taos, page, robot_id)
            .await
            .unwrap();
        rows
    };

    let mut from_headers = vec![];
    for item in rt.get(0).unwrap() {
        from_headers.push(item.to_string())
    }
    if rt.len() == 1 {
        (Vec::new(), count.unwrap(), from_headers)
    } else {
        (rt.split_off(1), count.unwrap(), from_headers)
    }
}

#[allow(non_snake_case)]
fn App() -> Element {
    let page = PAGE.lock().unwrap().get();
    let stables = get_stables();

    let nav_width = SIZE.0;
    let table_width = SIZE.1;
    let table_data_state: Signal<TableData> = use_signal(|| {
        DATA.lock().unwrap().take().unwrap()
    });

    *TIMES.lock().unwrap().get_mut() += 1;

    let propsa = StablesList {
        width: nav_width,
        stables,
        on_stable_change: EventHandler::new({
            move |msg: String| {
                spawn(message_handler(Message::ChangeStable(msg, table_width, table_data_state.clone())));
            }
        }),
        on_host_change: EventHandler::new({
            move |ip: String| {
                let stable = CURRENT_STABLE.lock().unwrap().get_mut().clone();
                turn_taos(ip);
                spawn(message_handler(Message::ChangeStable(stable, table_width, table_data_state.clone())));
            }
        }),
    };
    let propsb = TableList {
        width: table_width,
        table_data: table_data_state.clone(),
        on_search: EventHandler::new({
            move |msg: String| {
                spawn(message_handler(Message::StableFilter(msg, table_data_state.clone())));
            }
        }),
        on_resize: EventHandler::new({
            move |(index, moving_size)| {
                spawn(message_handler(Message::Resizing(table_width, index, moving_size, table_data_state.clone())));
            }
        }),
        on_resize_over: EventHandler::new({
            move |_| {
                spawn(message_handler(Message::ResizeOver(table_data_state.clone())));
            }
        }),
        on_refresh: EventHandler::new({
            move |msg: String| {
                spawn(message_handler(Message::StableFilter(msg, table_data_state.clone())));
            }
        }),
    };
    rsx! {
        div {
            class: "flex p-1",
            font_family: "hack",
            Stables {
                props: propsa,
            }
            Table {
                props: propsb,
            }
        }
        div {
            class: "flex justify-end p-1",
            div {
                button {
                    class: "mr-2 bg-sky-300 hover:bg-sky-500 text-white font-bold py-2 px-4 rounded",
                    onclick: {
                        move |_| {
                            spawn(message_handler(Message::PrevPage(table_data_state.clone())));
                        }
                    },
                    "上一页"
                }
            }
            div {
                button {
                    class: "mr-2 bg-sky-300 hover:bg-sky-500 text-white font-bold py-2 px-4 rounded",
                    onclick: {
                        move |_| {
                            spawn(message_handler(Message::NextPage(table_data_state.clone())));

                        }
                    },
                    "下一页"
                }
            }
            div {
                button { class: "bg-sky-300 text-white font-bold py-2 px-4 rounded",
                    "Total{table_data_state.read().total_size} Per20 {page}/{table_data_state.read().total_page}"
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct TableList {
    table_data: Signal<TableData>,
    width: i64,
    on_search: EventHandler<String>,
    on_resize: EventHandler<(i64, i64)>,
    on_resize_over: EventHandler,
    on_refresh: EventHandler<String>,
}

#[allow(non_snake_case)]
#[component]
fn Table(props: TableList) -> Element {
    let mut robot_id_state = use_signal(|| "".to_string());
    rsx! {
        div { style: "width:{props.width}px",
            div { class: "flex",
                div { class: "basis-1/4",
                    input {
                        placeholder: "robotId",
                        class: "placeholder:italic placeholder:text-slate-400 block bg-white w-full border border-slate-300 rounded-md py-2 pl-9 pr-3 shadow-sm focus:outline-none focus:border-sky-500 focus:ring-sky-500 focus:ring-1 sm:text-sm",
                        oninput: move |evt| robot_id_state.set(evt.value())
                    }
                }
                div {
                    button {
                        class: "bg-sky-500 hover:bg-sky-700 text-white font-bold py-2 px-4 rounded",
                        onclick: {
                            let on_search = props.on_search.clone();
                            move |_| {
                                on_search.call(robot_id_state.to_string())
                            }
                        },
                        "搜索"
                    }
                }
                div {
                    class: "ml-2",
                    button {
                        class: "bg-red-500 hover:bg-red-700 text-white font-bold py-2 px-4 rounded",
                        onclick: {
                            let on_refresh = props.on_refresh.clone();
                            move |_| {
                             on_refresh.call(robot_id_state.to_string());
                            }
                        },
                        "刷新"
                    }
                }
                div { class: "text-rose-400 flex justify-center items-center ml-auto",
                    p { " {props.table_data.read().spend}ms" }
                }
            }
            table {
                class: "border border-slate-400 text-gray-600 ",
                table_layout: "fixed",
                border: "1",
                width: "100%",
                thead {
                    tr {
                        for (index , header) in props.table_data.read().headers.iter().enumerate() {
                            td {
                                class: "border border-slate-300 bg-sky-500 text-white text-left hover:cursor-pointer overflow-clip",
                                style: "txt-overflow: ellipsis;white-space: nowrap;",
                                onmousedown: |_| {
                                    let window = dioxus_desktop::use_window();
                                    RESIZING.lock().unwrap().set(true);
                                    X.lock().unwrap().set(window.cursor_position().unwrap().x);
                                },
                                onmousemove: {
                                    let on_resize = props.on_resize.clone();
                                    move |_e| {
                                        if RESIZING.lock().unwrap().get() {
                                            let window = dioxus_desktop::use_window();
                                            let move_size = window.cursor_position().unwrap().x
                                                - X.lock().unwrap().get();
                                            on_resize.call((index as i64, move_size as i64));
                                        }
                                    }
                                },
                                onmouseup: {
                                    let on_resize_over = props.on_resize_over.clone();
                                    move |_| {
                                        RESIZING.lock().unwrap().set(false);
                                        on_resize_over.call(());
                                    }
                                },
                                onmouseleave: {
                                    let on_resize_over = props.on_resize_over.clone();
                                    move |_| {
                                        if RESIZING.lock().unwrap().get() {
                                            RESIZING.lock().unwrap().set(false);
                                            on_resize_over.call(());
                                        }
                                    }
                                },
                                " {header}"
                            }
                        }
                    }
                }
                colgroup {
                    for (index , _row) in props.table_data.read().headers.iter().enumerate() {
                        col { style: "width:{props.table_data.read().widths.get(index).unwrap()}px" }
                    }
                }
                tbody {
                    for row in props.table_data.read().rows.iter() {
                        tr {
                            for (_index , cell) in row.iter().enumerate() {
                                td {
                                    class: "border border-slate-300 overflow-clip text-left",
                                    style: "txt-overflow: ellipsis;white-space: nowrap;",
                                    "{cell}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Props, PartialEq, Clone)]
struct StablesList {
    width: i64,
    stables: Vec<String>,
    on_stable_change: EventHandler<String>,
    on_host_change: EventHandler<String>,
}

#[allow(non_snake_case)]
#[component]
fn Stables(props: StablesList) -> Element {
    let f = |c: String| {
        if c == CURRENT_STABLE.lock().unwrap().get_mut().clone() {
            "border-b  cursor-pointer hover:bg-gray-200 p-2 border-l-4 border-sky-500 ..."
                .to_string()
        } else {
            "border-b  cursor-pointer hover:bg-gray-200 p-2 ".to_string()
        }
    };
    rsx! {
        div {
            style: "width:{props.width}px",
            select {
                class: "form-select w-full",
                onchange: {
                    let on_host_change = props.on_host_change.clone();
                    move |e: Event<FormData>| {
                        on_host_change.call(e.value());
                    }
                },
                for conf in CONF.sources.iter() {
                    option {
                         "{conf.ip.clone()}"
                    }
                }
            }
            div {
                class: "list-none border",
                for stable in props.stables.iter() {
                    div {
                        class: "flex w-full",
                        li {
                            class: "{f(stable.clone())} text-gray-600 w-full",
                            onclick: {
                                let on_stable_change = props.on_stable_change.clone();
                                let stable = stable.clone();
                                move |_evt| {
                                    on_stable_change.call(stable.clone());
                                }
                            },
                            " {stable}"
                        }
                    }
                }
            }
        }
    }
}

// connect to taos

#[derive(Debug, Clone)]
pub struct TableData {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    total_size: i64,
    total_page: i64,
    changed_size: Vec<i64>,
    real_moving_size: Vec<i64>,
    widths: Vec<i64>,
    spend: String,
}

pub struct HostData {
    ip: String,
    port: usize,
    local_port: Option<usize>,
    ssh_user: Option<String>,
    password: Option<String>,
    taos: Option<Taos>,
    db: String,
    stables: Vec<String>,
}

async fn connect_host(config: Source) -> HostData {
    let mut host_data = HostData {
        ip: config.ip,
        port: config.port,
        local_port: config.local_port, // ssh -nl 本地端口
        ssh_user: config.ssh_user,
        password: config.ssh_password,
        db: config.db,
        taos: None,
        stables: Vec::new(),
    };

    // 开启ssh隧道
    if host_data.ssh_user.is_some() {
        let mut command = std::process::Command::new("/usr/bin/ssh");
        command.args([
            "-NL",
            format!(
                "{}:127.0.0.1:{}",
                host_data.local_port.unwrap(),
                host_data.port
            )
                .as_str(),
            format!("{}@{}", host_data.ssh_user.clone().unwrap(), host_data.ip).as_str(),
        ]);
        command.spawn().unwrap();
    }
    connect_taos(&mut host_data).await;
    host_data
}

async fn connect_taos(host_data: &mut HostData) {
    let dsn: String;
    if host_data.ssh_user.is_some() {
        dsn = format!("taos://127.0.0.1:{}", host_data.local_port.unwrap());
    } else {
        dsn = format!("taos://{}:6030", host_data.ip);
    }
    let builder = TaosBuilder::from_dsn(dsn).unwrap();
    let taos = builder.build().await.unwrap();
    taos.use_database(&host_data.db).await.unwrap();
    let stables = td::STable::get_stables(&taos).await.unwrap();
    let stables = stables
        .iter()
        .map(|item| item.stable_name.clone())
        .collect::<Vec<String>>();

    host_data.stables = stables;
    host_data.taos = Some(taos);
}

fn turn_taos(ip: String) {
    {
        let mut index = 0;
        let mut taos = TAOS.try_lock().unwrap();
        let mut taos_data = taos.take().unwrap();
        for (i, each) in taos_data.iter().enumerate() {
            if each.ip == ip {
                index = i;
                break;
            }
        }
        taos_data.swap(0, index);
        let _ = taos.set(taos_data);
    }
    print_current_host();
}

fn get_stables() -> Vec<String> {
    TAOS.lock()
        .unwrap()
        .get_mut()
        .unwrap()
        .index(0)
        .stables
        .clone()
}

fn print_current_host() {}
