use ingetrazo_render::source_texture_average::source_average_pixels;
fn main() {
    let args=std::env::args().collect::<Vec<_>>();
    if args.len()==5 {
        let raw=std::fs::read(&args[1]).unwrap();
        println!("{:?}",source_average_pixels(&raw,args[2].parse().unwrap(),args[3].parse().unwrap(),args[4].parse().unwrap()).unwrap());
        return;
    }
    for alpha in [true, false] {
        for w in [1, 2, 3, 4, 5, 7, 8, 9, 13, 32] {
            for h in [1, 2, 3, 4, 5, 7, 8, 9, 13, 32] {
                let mut rgba = vec![];
                for y in 0..h {
                    for x in 0..w {
                        rgba.extend([
                            (x * 37 + y * 17 + 1) as u8,
                            (x * 13 + y * 61 + 7) as u8,
                            (x * 91 + y * 3 + 19) as u8,
                            if alpha {
                                (x * 43 + y * 29 + 51) as u8
                            } else {
                                255
                            },
                        ]);
                    }
                }
                println!(
                    "{} {} {} {:?}",
                    w,
                    h,
                    alpha,
                    source_average_pixels(&rgba, w, h, alpha).unwrap()
                );
            }
        }
    }
}
