use crate::msg_sys::func_mod::gc_name::GCNAME;
use crate::msg_sys::func_mod::postgres_db::DBLINK;
use crate::msg_sys::func_mod::ttf::TTF;
use crate::msg_sys::func_msg::emoji_photo::MemPhoto;
use crate::msg_sys::func_msg::emoji_say::EmoMjk;
use crate::msg_sys::func_msg::game_center_manage::GameCenterManager;
use crate::msg_sys::func_msg::game_center_manage::gcm_add::GCMAdd;
use crate::msg_sys::func_msg::game_center_manage::gcm_add_name::GCMAddName;
use crate::msg_sys::func_msg::game_center_manage::gcm_bind::GCMBind;
use crate::msg_sys::func_msg::game_center_manage::gcm_delete::GCMDelete;
use crate::msg_sys::func_msg::game_center_manage::gcm_delete_name::GCMDeleteName;
use crate::msg_sys::func_msg::game_center_manage::gcm_query_gc::GCMQueryGc;
use crate::msg_sys::func_msg::game_center_manage::gcm_rename::GCMRename;
use crate::msg_sys::func_msg::game_center_manage::gcm_rewrite_description::GCMReDescription;
use crate::msg_sys::func_msg::game_center_manage::gcm_search::GCMSearch;
use crate::msg_sys::func_msg::game_center_manage::gcm_unbind::GCMUnbind;
use crate::msg_sys::func_msg::gc_query_report::GCQR;
use crate::msg_sys::func_msg::gc_query_report::gcqr_query::GcqrQuery;
use crate::msg_sys::func_msg::gc_query_report::gcqr_repo_plus::GcqrPlus;
use crate::msg_sys::func_msg::gc_query_report::gcqr_report_minus::GcqrMinus;
use crate::msg_sys::func_msg::gc_query_report::gcqr_report_set::GcqrSet;
use crate::msg_sys::func_msg::help::Help;
use crate::msg_sys::func_msg::play::Play;
use crate::msg_sys::func_msg::plusone::PlusOne;
use crate::msg_sys::func_msg::test::Test;
use crate::msg_sys::func_msg::ttt::TTT;
use crate::msg_sys::func_notice::group_decrease::Decrease;
use crate::msg_sys::func_notice::group_increase::Increase;
use crate::msg_sys::func_notice::poke::Poke;
use crate::msg_sys::msg_sys::{FnHandler, ModHandler};
use dashmap::DashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicUsize};

pub fn mod_handler_regin() -> Vec<&'static (dyn ModHandler + Send + Sync)> {
    let handlers: Vec<&'static (dyn ModHandler + Send + Sync)> = vec![&*TTF, &*DBLINK, &*GCNAME];
    handlers
}

//msg功能注册函数
pub fn msg_handler_regin() -> Vec<Box<dyn FnHandler + Send + Sync>> {
    let handlers: Vec<Box<dyn FnHandler + Send + Sync>> = vec![
        Box::new(Test {
            status: AtomicBool::from(false),
        }),
        Box::new(EmoMjk {
            enable: true,
            status: AtomicBool::from(false),
        }),
        Box::new(Play {
            status: AtomicBool::from(false),
        }),
        Box::new(Help {
            status: AtomicBool::from(false),
        }),
        Box::new(TTT {
            enable: false,
            status: AtomicBool::from(false),
            data_map: OnceLock::from(DashMap::new()),
            data_map_status: AtomicUsize::new(0),
        }),
        Box::new(MemPhoto {
            status: AtomicBool::from(false),
        }),
        Box::new(GameCenterManager {
            enable: true,
            status: Default::default(),
            subfunction: vec![
                Box::new(GCMAdd {
                    status: AtomicBool::from(false),
                }),
                Box::new(GCMDelete {
                    status: AtomicBool::from(false),
                }),
                Box::new(GCMRename {
                    status: AtomicBool::from(false),
                }),
                Box::new(GCMAddName {
                    status: AtomicBool::from(false),
                }),
                Box::new(GCMBind {
                    status: AtomicBool::from(false),
                }),
                Box::new(GCMUnbind {
                    status: AtomicBool::from(false),
                }),
                Box::new(GCMDeleteName {
                    status: AtomicBool::from(false),
                }),
                Box::new(GCMSearch {
                    status: AtomicBool::from(false),
                }),
                Box::new(GCMReDescription {
                    status: AtomicBool::from(false),
                }),
                Box::new(GCMQueryGc {
                    status: AtomicBool::from(false),
                }),
            ],
        }),
        Box::new(GCQR {
            status: AtomicBool::from(false),
            sub_function: vec![
                Box::new(GcqrQuery {
                    status: AtomicBool::from(false),
                }),
                Box::new(GcqrSet {
                    status: AtomicBool::from(false),
                }),
                Box::new(GcqrPlus {
                    status: AtomicBool::from(false),
                }),
                Box::new(GcqrMinus {
                    status: AtomicBool::from(false),
                }),
            ],
        }),
        Box::new(PlusOne {
            status: AtomicBool::from(false),
            map: OnceLock::new(),
        }),
    ];
    handlers
}
//notice功能注册函数
pub fn notice_handler_regin() -> Vec<Box<dyn FnHandler + Send + Sync>> {
    let handlers: Vec<Box<dyn FnHandler + Send + Sync>> = vec![
        Box::new(Poke {
            status: AtomicBool::from(false),
        }),
        Box::new(Decrease { enable: true }),
        Box::new(Increase { enable: true }),
    ];
    handlers
}
