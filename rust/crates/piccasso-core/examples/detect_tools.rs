//! Print the colour pickers found on a screenshot.
//! cargo run -p piccasso-core --example detect_tools -- SCREENSHOT.png
fn main() {
    let path = std::env::args().nth(1).expect("screenshot path");
    let img = piccasso_core::decode_image_bytes(&std::fs::read(path).expect("read")).expect("decode");
    for a in piccasso_core::tooldetect::detect_color_widgets(&img) {
        println!("{a:?} {:?}", piccasso_core::tooldetect::picker_kind(&img, a));
    }
}
