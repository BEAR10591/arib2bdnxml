#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(unnecessary_transmutes)] // bindgen-generated code

include!(concat!(env!("OUT_DIR"), "/ffmpeg.rs"));
