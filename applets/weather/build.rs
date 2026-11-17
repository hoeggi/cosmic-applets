use std::{env, fs, path::Path};
use xdgen::{App, Context, FluentString};

/// Expands the translated .desktop / metainfo files into `generated/`
/// (packaged by `cargo deb`, see `[package.metadata.deb]`).
fn main() {
    for path in [
        "i18n",
        "resources/app.desktop",
        "resources/app.metainfo.xml",
    ] {
        println!("cargo::rerun-if-changed={path}");
    }

    let ctx = Context::new("i18n", env::var("CARGO_PKG_NAME").unwrap()).unwrap();
    let app = App::new(FluentString("app-title"))
        .comment(FluentString("app-comment"))
        .keywords(FluentString("app-keywords"));

    let desktop_entry = app.expand_desktop("resources/app.desktop", &ctx).unwrap();
    let metainfo = app
        .expand_metainfo("resources/app.metainfo.xml", &ctx)
        .unwrap();

    let output = Path::new("generated");
    fs::create_dir_all(output).unwrap();
    fs::write(output.join("app.desktop"), desktop_entry).unwrap();
    fs::write(output.join("app.metainfo.xml"), metainfo).unwrap();
}
