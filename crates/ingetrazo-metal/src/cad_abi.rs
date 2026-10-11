//! Shared native shader upload layouts, checked on every host architecture.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct DrawPush {
    pub scale: [f32; 2],
    pub translation: [f32; 2],
    pub screen: [f32; 2],
    pub pad: [f32; 2],
    pub rgba: [f32; 4],
    pub glyph: [u32; 4],
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_draw_upload_is_64_bytes_and_plain_cad_has_no_glyph_operator() {
        assert_eq!(std::mem::size_of::<DrawPush>(), 64);
        assert_eq!(std::mem::offset_of!(DrawPush, rgba), 32);
        assert_eq!(std::mem::offset_of!(DrawPush, glyph), 48);
        let push = DrawPush {
            scale: [1.; 2],
            translation: [0.; 2],
            screen: [800., 600.],
            pad: [0.; 2],
            rgba: [1.; 4],
            glyph: [0; 4],
        };
        let bytes = bytemuck::bytes_of(&push);
        assert_eq!(bytes.len(), 64);
        assert_eq!(&bytes[48..], &[0; 16]);
    }
}
