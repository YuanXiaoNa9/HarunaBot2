use crate::msg_sys::msg_sys::{FnHandler, Msg};
use anyhow::{Error, anyhow};
use anyhow_trace::anyhow_trace;
use async_trait::async_trait;
use dashmap::DashMap;
use dashmap::mapref::one::{Ref, RefMut};
use std::sync::OnceLock;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use tracing::debug;

pub struct TTT {
    pub enable: bool,
    pub status: AtomicBool,
    pub data_map: OnceLock<DashMap<i64, Vec<ChessData>>>,
    pub data_map_status: AtomicUsize,
}
pub struct ChessData {
    chess_player: i16,
    chess_hand: i16,
}
#[async_trait]
impl FnHandler for TTT {
    async fn matches(&self, msg: &Msg) -> bool {
        let mut splits = msg.raw_message.split(" ");
        if (splits.next() == Some("[CQ:at,qq=1246137523]")
            && splits.next() == Some("/井字棋")
            && splits.next().is_some())
            || (self.data_map_status.load(Relaxed) != 0
                && msg
                    .raw_message
                    .contains(&['1', '2', '3', '4', '5', '6', '7', '8', '9'])
                && self
                    .data_map
                    .get()
                    .unwrap()
                    .contains_key(&msg.sender.user_id))
        {
            debug!("TTT is OK");
            return true;
        }

        false
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        let mut splits = msg.raw_message.split(" ");
        splits.next();
        let cmd = splits.next();
        if cmd.is_some_and(|str| str == "开始") {
            if self
                .data_map
                .get()
                .unwrap()
                .contains_key(&msg.sender.user_id)
            {
                return Err(anyhow!("已有正在进行的对局"));
            }
            self.data_map
                .get()
                .unwrap()
                .insert(msg.user_id, init_data());
            self.data_map_status.fetch_add(1, Relaxed);
        } else if cmd.is_some_and(|str| str == "取消") {
            if !self
                .data_map
                .get()
                .unwrap()
                .contains_key(&msg.sender.user_id)
            {
                return Err(anyhow!("没有正在进行的对局"));
            }
            self.data_map.get().unwrap().remove(&msg.user_id)?;
            self.data_map_status.fetch_sub(1, Relaxed);
            return Ok(());
        }
        ttt_main_sys(msg, self.data_map.get().unwrap())?;
        Ok(())
    }

    async fn init(&self) {
        self.status.store(true, Relaxed);
    }

    async fn status(&self) -> bool {
        if self.enable && self.status.load(Relaxed) {
            return true;
        }
        false
    }
    async fn help(&self, _: &str) -> String {
        "开发中".to_string()
    }

    async fn name(&self) -> String {
        "井字棋".to_string()
    }
}
fn init_data() -> Vec<ChessData> {
    let mut new_vec = Vec::new();
    for _i in 0..9 {
        new_vec.push(ChessData {
            chess_player: 0,
            chess_hand: 0,
        });
    }
    new_vec
}
fn ttt_main_sys(msg: &Msg, chess_data: &DashMap<i64, Vec<ChessData>>) -> Result<(), Error> {
    let index = msg.raw_message.parse::<usize>()? - 1;
    chess_check(index, chess_data.get(&msg.sender.user_id).unwrap())?;
    chess_refresh(chess_data.get_mut(&msg.sender.user_id).unwrap());
    chess_update(index, chess_data.get_mut(&msg.sender.user_id).unwrap(), 1);
    let weight = chess_judgment(chess_data.get(&msg.sender.user_id).unwrap());
    let best = chess_biggest(weight);
    chess_update(best, chess_data.get_mut(&msg.sender.user_id).unwrap(), 8);
    Ok(())
}

fn chess_biggest(weights: Vec<usize>) -> usize {
    let mut best = 0;
    for i in 0..9 {
        if weights[i] > best {
            best = i
        }
    }
    best
}

fn chess_judgment(cd: Ref<i64, Vec<ChessData>>) -> Vec<usize> {
    let mut vec_new = Vec::new();
    for i in 0..9 {
        if cd[i].chess_player != 0 {
            vec_new.push(0);
            continue;
        }
        let count: AtomicUsize = AtomicUsize::new(0);
        let (x, y) = chess_xy(i);
        //横向
        if cd[chess_yx(1, y)].chess_player
            + cd[chess_yx(2, y)].chess_player
            + cd[chess_yx(3, y)].chess_player
            == 2
        {
            count.fetch_add(100, Relaxed);
        }
        //横向
        if cd[chess_yx(1, y)].chess_player
            + cd[chess_yx(2, y)].chess_player
            + cd[chess_yx(3, y)].chess_player
            == 8
        {
            count.fetch_add(80, Relaxed);
        }
        //横向
        if cd[chess_yx(x, 1)].chess_player
            + cd[chess_yx(x, 2)].chess_player
            + cd[chess_yx(x, 3)].chess_player
            == 2
        {
            count.fetch_add(100, Relaxed);
        }
        //横向
        if cd[chess_yx(x, 1)].chess_player
            + cd[chess_yx(x, 2)].chess_player
            + cd[chess_yx(x, 3)].chess_player
            == 8
        {
            count.fetch_add(80, Relaxed);
        }
        //斜向（为角时）
        if [0, 2, 6, 8].contains(&(i))
            && cd[chess_yx(1, 1)].chess_player
                + cd[chess_yx(2, 2)].chess_player
                + cd[chess_yx(3, 3)].chess_player
                == 2
        {
            count.fetch_add(100, Relaxed);
        }
        //斜向（为角时）
        if [0, 2, 6, 8].contains(&(i))
            && cd[chess_yx(1, 1)].chess_player
                + cd[chess_yx(2, 2)].chess_player
                + cd[chess_yx(3, 3)].chess_player
                == 8
        {
            count.fetch_add(80, Relaxed);
        }
        //斜向（为中心时）
        if i == 5
            && (cd[chess_yx(1, 1)].chess_player
                + cd[chess_yx(2, 2)].chess_player
                + cd[chess_yx(3, 3)].chess_player
                == 2
                || cd[chess_yx(3, 1)].chess_player
                    + cd[chess_yx(2, 2)].chess_player
                    + cd[chess_yx(1, 3)].chess_player
                    == 2)
        {
            count.fetch_add(100, Relaxed);
        }
        //斜向（为中心时）
        if i == 5
            && (cd[chess_yx(3, 1)].chess_player
                + cd[chess_yx(2, 2)].chess_player
                + cd[chess_yx(1, 3)].chess_player
                == 8
                || cd[chess_yx(1, 1)].chess_player
                    + cd[chess_yx(2, 2)].chess_player
                    + cd[chess_yx(3, 3)].chess_player
                    == 8)
        {
            count.fetch_add(80, Relaxed);
        }
        if [1, 3, 5, 7].contains(&(i)) {
            let i = count.load(Relaxed);
            count.store(i * 2, Relaxed);
        } else if [0, 2, 4, 6, 8].contains(&(i)) {
            let i = count.load(Relaxed);
            count.store(i * 3, Relaxed);
        } else {
            let i = count.load(Relaxed);
            count.store(i * 4, Relaxed);
        }
        vec_new.push(count.load(Relaxed));
    }
    vec_new
}

fn chess_xy(i: usize) -> (usize, usize) {
    for x in 1..3 {
        for y in 1..3 {
            if x + (y - 1) * 3 == i + 1 {
                return (x, y);
            }
        }
    }
    (0, 0)
}
fn chess_yx(x: usize, y: usize) -> usize {
    (x + (y - 1) * 3) - 1
}

fn chess_refresh(mut chess_data: RefMut<i64, Vec<ChessData>>) {
    for i in 0..9 {
        if chess_data[i].chess_player == 0 || chess_data[i].chess_hand == 3 {
            chess_data[i] = ChessData {
                chess_player: 0,
                chess_hand: 0,
            };
        } else {
            chess_data[i] = ChessData {
                chess_player: chess_data[i].chess_player,
                chess_hand: chess_data[i].chess_hand + 1,
            };
        }
    }
}

fn chess_update(i: usize, mut chess_data: RefMut<i64, Vec<ChessData>>, player: i16) {
    chess_data[i] = ChessData {
        chess_player: player,
        chess_hand: 1,
    };
}

fn chess_check(i: usize, chess_data: Ref<i64, Vec<ChessData>>) -> Result<(), Error> {
    if chess_data[i].chess_player == 0 && chess_data[i].chess_hand == 0 {
        return Ok(());
    }
    Err(anyhow!("此处已有棋子"))
}
