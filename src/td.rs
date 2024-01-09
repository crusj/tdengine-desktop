use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use taos::*;
use taos::BorrowedValue::BigInt;

#[derive(Debug, serde::Deserialize)]
pub struct Status {
    pub ts: DateTime<Local>,
    pub status: Option<i64>,
    pub robot_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct STable {
    pub stable_name: String,
}

impl STable {
    pub fn new(stable_name: String) -> STable {
        STable { stable_name }
    }
    // 获取所有超表
    pub async fn get_stables(taos: &Taos) -> Result<Vec<STable>> {
        let mut stables = taos
            .query("show stables")
            .await?
            .deserialize()
            .try_collect::<Vec<STable>>()
            .await?;

        stables.sort_by(|a, b| a.stable_name.cmp(&b.stable_name));

        Ok(stables)
    }

    // 获取超表下的子表
    #[allow(unused)]
    pub async fn get_sub_tables(&self, taos: &Taos) -> Result<Vec<Table>> {
        let name = self
            .stable_name
            .split('_')
            .nth(1)
            .context("error stable name")?;
        let sql = format!("{}_%", name);
        let mut sub_tables = taos
            .query(format!("show tables like \"{}\"", sql))
            .await?
            .deserialize()
            .try_collect::<Vec<Table>>()
            .await?;
        sub_tables.sort_by(|a, b| a.table_name.cmp(&b.table_name));

        Ok(sub_tables)
    }

    // 获取超表下的数据
    pub async fn get_rows(&self, taos: &Taos, page: i32, robot_id: Option<String>) -> Result<(Vec<Vec<String>>, Option<i64>)> {
        let offset = (page - 1) * 20;
        // robot_id
        let robot_id_where = if let Some(robot_id) = robot_id {
            format!("where robot_id like \"%{}%\"", robot_id)
        } else {
            "".to_string()
        };


        // 查询总的记录树
        let count_sql = format!(
            "select count(*) as c from {} {}",
            self.stable_name, robot_id_where
        );
        println!("{}", count_sql);
        let mut count_result = taos.query(count_sql).await?;
        let mut total_size = 0;
        if let Some(row) = count_result.rows().try_next().await? {
            for (_, value) in row {
                if let BigInt(value) = value {
                    total_size = value;
                }
            }
        };

        let sql = format!(
            "select * from {} {} order by ts desc limit 20 offset {}",
            self.stable_name, robot_id_where, offset
        );
        println!("{}", sql);
        let mut result = taos.query(sql).await?;

        let fields = result
            .fields()
            .iter()
            .map(|v| v.name().to_string())
            .collect::<Vec<String>>();

        let mut list = Vec::new();
        list.push(fields);
        let mut rows = result.rows();
        while let Some(row) = rows.try_next().await? {
            let mut data = Vec::new();
            for (_, value) in row {
                match value {
                    BorrowedValue::Timestamp(value) => {
                        let ts = value
                            .to_datetime_with_tz()
                            .format("%Y-%m-%d %H:%m:%S")
                            .to_string();
                        data.push(ts);
                    }
                    _ => {
                        data.push(value.to_string().unwrap());
                    }
                }
            }
            list.push(data);
        }

        Ok((list, Some(total_size)))
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct Table {
    pub table_name: String,
}

impl Table {}
