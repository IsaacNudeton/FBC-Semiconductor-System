// Auto-generated PIP Database (ONETWO learned)
// DO NOT EDIT - regenerate with pip_analyzer

use std::collections::HashMap;
use lazy_static::lazy_static;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoutingDirection {
    Local, Short, Medium, Long, InterCol,
}

lazy_static! {
    pub static ref PIP_DATABASE: HashMap<(u16, RoutingDirection), u32> = {
        let mut m = HashMap::new();
        m.insert((4, RoutingDirection::Local), 0x000004E2);
        m.insert((5, RoutingDirection::Local), 0x00000003);
        m.insert((5, RoutingDirection::Short), 0x000014A2);
        m.insert((6, RoutingDirection::Short), 0x00005D45);
        m.insert((6, RoutingDirection::Local), 0x00001007);
        m.insert((7, RoutingDirection::Local), 0x00001007);
        m.insert((8, RoutingDirection::Short), 0x000013B1);
        m.insert((8, RoutingDirection::Local), 0x0000132B);
        m.insert((9, RoutingDirection::Short), 0x000014BA);
        m.insert((9, RoutingDirection::Local), 0x00000102);
        m.insert((10, RoutingDirection::Short), 0x00001306);
        m.insert((10, RoutingDirection::Local), 0x00001001);
        m.insert((11, RoutingDirection::Local), 0x00001004);
        m.insert((13, RoutingDirection::Local), 0x000013D3);
        m.insert((13, RoutingDirection::Short), 0x00001AE8);
        m.insert((14, RoutingDirection::Medium), 0x000003E6);
        m.insert((14, RoutingDirection::Local), 0x000003FD);
        m.insert((15, RoutingDirection::Medium), 0x0000151B);
        m.insert((15, RoutingDirection::Short), 0x000004A0);
        m.insert((15, RoutingDirection::Local), 0x00000523);
        m.insert((16, RoutingDirection::Short), 0x00001446);
        m.insert((16, RoutingDirection::Local), 0x00001446);
        m.insert((16, RoutingDirection::Medium), 0x00001446);
        m.insert((17, RoutingDirection::Medium), 0x00000747);
        m.insert((17, RoutingDirection::Long), 0x0000104C);
        m.insert((17, RoutingDirection::Local), 0x00000954);
        m.insert((18, RoutingDirection::Local), 0x000007E0);
        m.insert((18, RoutingDirection::Medium), 0x00000019);
        m.insert((18, RoutingDirection::Long), 0x000007E0);
        m.insert((19, RoutingDirection::Short), 0x00000A5B);
        m.insert((19, RoutingDirection::Medium), 0x00000300);
        m.insert((19, RoutingDirection::Local), 0x00000300);
        m.insert((20, RoutingDirection::Local), 0x6A001001);
        m.insert((24, RoutingDirection::Local), 0x0000176A);
        m.insert((25, RoutingDirection::Short), 0x00001B83);
        m.insert((25, RoutingDirection::Local), 0x000016CE);
        m.insert((26, RoutingDirection::Short), 0x000018FD);
        m.insert((26, RoutingDirection::Local), 0x00000514);
        m.insert((27, RoutingDirection::Local), 0x000013BF);
        m.insert((28, RoutingDirection::Local), 0x00080877);
        m.insert((29, RoutingDirection::Short), 0x008410F9);
        m.insert((29, RoutingDirection::Local), 0x00001F20);
        m.insert((30, RoutingDirection::Local), 0x00001456);
        m.insert((31, RoutingDirection::Local), 0x00001EB2);
        m.insert((33, RoutingDirection::Short), 0x00001001);
        m.insert((33, RoutingDirection::Local), 0x00001BEF);
        m.insert((34, RoutingDirection::Local), 0x00000DE0);
        m.insert((35, RoutingDirection::Local), 0x00001793);
        m.insert((35, RoutingDirection::Short), 0x0000048F);
        m.insert((36, RoutingDirection::Local), 0x00001CE1);
        m.insert((36, RoutingDirection::Short), 0x00001946);
        m.insert((37, RoutingDirection::Local), 0x00000D9F);
        m.insert((37, RoutingDirection::Short), 0x00000B1A);
        m.insert((38, RoutingDirection::Local), 0x00001004);
        m.insert((38, RoutingDirection::Short), 0x00001AD7);
        m.insert((39, RoutingDirection::Short), 0x00000036);
        m.insert((39, RoutingDirection::Local), 0x00000009);
        m.insert((40, RoutingDirection::Local), 0xB807D9B7);
        m.insert((40, RoutingDirection::Short), 0x0B80160D);
        m.insert((41, RoutingDirection::Local), 0x00001A03);
        m.insert((42, RoutingDirection::Short), 0x002040C6);
        m.insert((42, RoutingDirection::Local), 0x00400BFE);
        m.insert((43, RoutingDirection::Local), 0x002052D0);
        m.insert((44, RoutingDirection::Local), 0x00000BF2);
        m.insert((44, RoutingDirection::Short), 0x000005C7);
        m.insert((45, RoutingDirection::Local), 0x00000617);
        m.insert((46, RoutingDirection::Local), 0x0000174D);
        m.insert((46, RoutingDirection::Short), 0x040052A7);
        m.insert((47, RoutingDirection::Local), 0x00001506);
        m.insert((48, RoutingDirection::Local), 0x000018B1);
        m.insert((49, RoutingDirection::Local), 0x000011B4);
        m.insert((50, RoutingDirection::Local), 0x0000085D);
        m.insert((51, RoutingDirection::Local), 0x00000372);
        m.insert((52, RoutingDirection::Local), 0x00001F17);
        m.insert((53, RoutingDirection::Medium), 0x04005DCA);
        m.insert((53, RoutingDirection::Local), 0x000017F8);
        m.insert((54, RoutingDirection::Local), 0x00001629);
        m.insert((55, RoutingDirection::Local), 0x00001433);
        m.insert((56, RoutingDirection::Short), 0x00000FDF);
        m.insert((56, RoutingDirection::Local), 0x00001344);
        m.insert((57, RoutingDirection::Local), 0x0000020D);
        m.insert((57, RoutingDirection::Short), 0x000018AD);
        m.insert((58, RoutingDirection::Local), 0x00000226);
        m.insert((59, RoutingDirection::Local), 0x00001C0B);
        m.insert((60, RoutingDirection::Local), 0x6A001001);
        m.insert((60, RoutingDirection::Short), 0x0B801DC7);
        m
    };
}
