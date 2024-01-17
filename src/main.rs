use std::cell::{Cell, OnceCell};
use std::io::prelude::*;
use std::ops::{Index, IndexMut};
use std::rc::Rc;
use std::string::ToString;
use std::sync::Mutex;

use dioxus::prelude::*;
use dioxus_desktop::WindowBuilder;
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

lazy_static! {
    static ref CURRENT_STABLE: Mutex<Cell<String>> = Mutex::new(Cell::new(String::default()));
    static ref PAGE: Mutex<Cell<i32>> = Mutex::new(Cell::new(1));
    static ref ROBOT_ID: Mutex<Cell<String>> = Mutex::new(Cell::new(String::default()));
}

fn main() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let a = runtime.handle();
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
    });

    // ssh_tunnel(TEST_HOST.to_string(), TEST_USER.to_string());
    dioxus_desktop::launch_with_props(
        App,
        AppProps {
            runtime: Rc::new(runtime),
        },
        dioxus_desktop::Config::new()
            .with_custom_head(r#"<link rel="stylesheet" href="public/tailwind.css">"#.to_string())
            .with_window(WindowBuilder::new().with_resizable(true).with_inner_size(
                dioxus_desktop::wry::application::dpi::LogicalSize::new(1800.0, 1200.0),
            )),
    );
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
    println!("{:?}, {:?}", rt, count.clone().unwrap());
    if rt.len() == 1 {
        (Vec::new(), count.unwrap(), from_headers)
    } else {
        (rt.split_off(2), count.unwrap(), from_headers)
    }
}

struct AppProps {
    runtime: Rc<tokio::runtime::Runtime>,
}

#[allow(non_snake_case)]
fn App(cx: Scope<AppProps>) -> Element {
    let window_size = dioxus_desktop::use_window(cx).inner_size();
    // 计算宽高
    // 1/5 4/5
    let nav_width = (window_size.width as f64 * 0.25) as i64;
    let table_width = window_size.width as i64 - nav_width;

    let table_data_state = use_state(cx, || {
        let start = std::time::Instant::now();
        let (rows, total_size, headers) = cx.props.runtime.block_on(async { get_rows().await });
        let l = headers.len();
        let changed_size = vec![0; headers.len()];
        let real_moving_size = vec![0; headers.len()];
        let total_page: i64;
        if total_size / PAGE_SIZE == 0 && total_size > PAGE_SIZE {
            total_page = total_size / PAGE_SIZE;
        } else {
            total_page = total_size / PAGE_SIZE + 1;
        };

        TableData {
            runtime: cx.props.runtime.clone(),
            headers,
            rows,
            total_size,
            total_page,
            changed_size: changed_size.clone(),
            real_moving_size: real_moving_size.clone(),
            widths: cal_widths(table_width, l as i64, changed_size, real_moving_size),
            spend: start.elapsed().as_millis().to_string(),
        }
    });

    let page = PAGE.lock().unwrap().get();
    let stables = get_stables();

    *TIMES.lock().unwrap().get_mut() += 1;
    cx.render(rsx! {
        div {
            class: "flex p-1",
            font_family: "hack",
            Stables {
                width: nav_width,
                stables: stables,
                on_stable_change: move |msg: String| { // stable change
                   message_handler(cx.props.runtime.clone(), Message::ChangeStable(msg, table_width,table_data_state.clone()))
                },
                on_host_change: move |ip: String| {
                   let stable = CURRENT_STABLE.lock().unwrap().get_mut().clone();
                   turn_taos(ip);
                   message_handler(cx.props.runtime.clone(),Message::ChangeStable(stable, table_width,table_data_state.clone()));
                }
            },
            Table {
                width: table_width,
                table_data: table_data_state.clone(),
                on_search: |msg: String| {
                    message_handler(cx.props.runtime.clone(),Message::StableFilter(msg,table_data_state.clone()));
                },
                on_resize: move |(index, moving_size)| {
                    message_handler(cx.props.runtime.clone(),Message::Resizing(table_width, index, moving_size, table_data_state.clone()));
                },
                on_resize_over: |_| {
                    message_handler(cx.props.runtime.clone(),Message::ResizeOver(table_data_state.clone()));
                }
            }
        }
        div {
            class:"flex justify-end p-1",
            div {
                button {
                    class: "mr-2 bg-sky-300 hover:bg-sky-500 text-white font-bold py-2 px-4 rounded",
                    onclick: |_| {
                        message_handler(cx.props.runtime.clone(),Message::PrevPage(table_data_state.clone()));
                    },
                    "上一页"
                }
            }
            div {
                button {
                    class: "mr-2 bg-sky-300 hover:bg-sky-500 text-white font-bold py-2 px-4 rounded",
                    onclick: move |_| {
                        message_handler(cx.props.runtime.clone(),Message::NextPage(table_data_state.clone()));
                    },
                    "下一页"
                }
            }
            div {
                button {
                    class: "bg-sky-300 text-white font-bold py-2 px-4 rounded",
                    "Total{table_data_state.total_size} Per20 {page}/{table_data_state.total_page}"
                }
            }
        }
    })
}

#[derive(Props)]
struct TableList<'a> {
    table_data: UseState<TableData>,
    width: i64,
    on_search: EventHandler<'a, String>,
    on_resize: EventHandler<'a, (i64, i64)>,
    on_resize_over: EventHandler<'a>,
}

#[allow(non_snake_case)]
fn Table<'a>(cx: Scope<'a, TableList<'a>>) -> Element<'a> {
    let robot_id_state = use_state(cx, || "".to_string());
    render! {
        div {
            style: "width:{cx.props.width}px",
            div {
                class: "flex",
                div {
                    class: "basis-1/4",
                    input {
                        placeholder: "robotId",
                        class: "placeholder:italic placeholder:text-slate-400 block bg-white w-full border border-slate-300 rounded-md py-2 pl-9 pr-3 shadow-sm focus:outline-none focus:border-sky-500 focus:ring-sky-500 focus:ring-1 sm:text-sm",
                        oninput: move |evt| robot_id_state.set(evt.value.clone()),
                    }
                }
                div {
                    button {
                        class: "bg-sky-500 hover:bg-sky-700 text-white font-bold py-2 px-4 rounded",
                        onclick: move |_| {
                            cx.props.on_search.call(robot_id_state.to_string());
                        },
                        "搜索"
                    }
                }
                div {
                    class: "text-rose-400 flex justify-center items-center ml-auto",
                    p {
                        " {cx.props.table_data.spend}ms"
                    }
                }
            }
            table {
                class: "border border-slate-400 text-gray-600 ",
                table_layout: "fixed",
                border: "1",
                width: "100%",

                thead {
                    tr {
                        for (index, header) in cx.props.table_data.headers.iter().enumerate() {
                            td {
                                class:"border border-slate-300 bg-sky-500 text-white text-left hover:cursor-pointer overflow-clip",
                                style:"txt-overflow: ellipsis;white-space: nowrap;",
                                onmousedown: |_| {
                                    let window = dioxus_desktop::use_window(cx);
                                    RESIZING.lock().unwrap().set(true);
                                    X.lock().unwrap().set(window.cursor_position().unwrap().x);
                                },
                                onmousemove:  {
                                    move |_e| {
                                        if RESIZING.lock().unwrap().get() {
                                            let window = dioxus_desktop::use_window(cx);
                                            let move_size = window.cursor_position().unwrap().x - X.lock().unwrap().get();
                                            cx.props.on_resize.call((index as i64, move_size as i64));
                                        }
                                    }
                                },
                                onmouseup: |_| {
                                    RESIZING.lock().unwrap().set(false);
                                    cx.props.on_resize_over.call(());

                                },
                                onmouseleave: |_| {
                                    if RESIZING.lock().unwrap().get() {
                                        RESIZING.lock().unwrap().set(false);
                                        cx.props.on_resize_over.call(());
                                    }
                                },
                                " {header}"
                            }
                        }
                    }
                }
                colgroup {
                    for (index, _row) in cx.props.table_data.headers.iter().enumerate() {
                        col {
                            style:"width:{cx.props.table_data.widths.get(index).unwrap()}px",
                        }
                    }
                }
                tbody {
                    for row in cx.props.table_data.rows.iter() {
                        tr {
                            for (_index,cell) in row.iter().enumerate() {
                                td {
                                    class:"border border-slate-300 overflow-clip text-left",
                                    style:"txt-overflow: ellipsis;white-space: nowrap;",
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

#[derive(Props)]
struct StablesList<'a> {
    width: i64,
    stables: Vec<String>,
    on_stable_change: EventHandler<'a, String>,
    on_host_change: EventHandler<'a, String>,
}

#[allow(non_snake_case)]
fn Stables<'a>(cx: Scope<'a, StablesList<'a>>) -> Element<'a> {
    let f = |c: String| {
        if c == CURRENT_STABLE.lock().unwrap().get_mut().clone() {
            "border-b  cursor-pointer hover:bg-gray-200 p-2 border-l-4 border-sky-500 ..."
                .to_string()
        } else {
            "border-b  cursor-pointer hover:bg-gray-200 p-2 ".to_string()
        }
    };
    render!(
        div {
            style: "width:{cx.props.width}px",
            select {
                class: "form-select w-full",
                onchange: |e: Event<FormData>| {
                    cx.props.on_host_change.call(e.value.clone());
                },
                for conf in CONF.sources.iter() {
                    rsx!(
                        option {
                            "{conf.ip.clone()}"
                        }
                    )
                },
            },
            div {
                class: "list-none border",
                for stable in cx.props.stables.iter() {
                    div {
                        class: "flex w-full",
                        li {
                            class: "{f(stable.clone())} text-gray-600 w-full",
                            onclick: move |_evt| {
                                cx.props.on_stable_change.call(stable.clone());
                            },
                            " {stable}",
                        }
                    }
                }
            }
        }
    )
}

// connect to taos

pub struct TableData {
    runtime: Rc<tokio::runtime::Runtime>,
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
        local_port: config.local_port,
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
    println!("===================={}", dsn);
    let builder = TaosBuilder::from_dsn(dsn).unwrap();
    let taos = builder.build().await.unwrap();
    println!("{}", &host_data.db);
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
        println!("=======================reverse taos start");
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
        println!("=======================reverse_taos end");
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

fn print_current_host() {
    println!(
        "=======================change host{}",
        TAOS.lock().unwrap().get_mut().unwrap().index(0).ip.clone()
    );
}
