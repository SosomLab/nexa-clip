fn main() {
    for name in ["JetBrains Mono", "JetBrainsMono", "Consolas"] {
        match nclip_plat::font::find_font_by_family(name) {
            Some((d, i)) => println!("{name}: ok ({} bytes, idx {i})", d.len()),
            None => println!("{name}: not found"),
        }
    }
}
