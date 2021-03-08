use kiss3d::nalgebra::DVector;

pub type ArrivalTimes = DVector<i64>;

#[repr(C)]
#[derive(Debug, Clone)]
pub(crate) struct Tag {
    type_: u8,
    missed_events: u16,
    channel: i32,
    time: i64,
}

// impl Tag {
//     pub(crate) unsafe fn from_ptr(ptr: *const u8) {
//         let record_as_slice = std::slice::from_raw_parts(&ptr, ITEMSIZE);
//         let type_ = u8::from_le_bytes(record_as_slice[0..1]);
//     }
// }


fn process_tags(time: Vec<Tag>) {
    todo!()
}



#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
