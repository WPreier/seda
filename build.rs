use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use pulldown_cmark::{html, CowStr, Event, Options, Parser, Tag};
use walkdir::WalkDir;

const STYLE: &str = r#":root{color-scheme:light dark;font-family:system-ui,sans-serif}body{max-width:72rem;margin:0 auto;padding:2rem;line-height:1.6}a{color:#3976c5}pre{overflow:auto;padding:1rem;background:#8882}code{font-family:ui-monospace,monospace}table{border-collapse:collapse}th,td{border:1px solid #888;padding:.35rem .6rem}img{max-width:100%}"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=assets/docs");
    println!("cargo:rerun-if-changed=assets/404.html");

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set"));
    let generated = out.join("content-docs");
    if generated.exists() {
        fs::remove_dir_all(&generated)?;
    }
    fs::create_dir_all(&generated)?;
    let style_path = generated.join("style.css");
    fs::write(&style_path, STYLE)?;

    let mut entries = Vec::new();
    for name in ["sedat", "seda"] {
        build_doc_tree(
            name,
            &Path::new("assets").join("docs").join(name),
            &generated,
            &style_path,
            &mut entries,
        )?;
    }

    let not_found = fs::canonicalize("assets/404.html")?;
    entries.push((
        "__404.html".to_owned(),
        "text/html; charset=utf-8",
        not_found,
    ));
    entries.sort_by(|left, right| left.0.cmp(&right.0));

    let mut module =
        String::from("pub(crate) static EMBEDDED_FILES: &[(&str, &str, &[u8])] = &[\n");
    for (path, mime, source) in entries {
        module.push_str(&format!(
            "    ({path:?}, {mime:?}, include_bytes!({source:?})),\n",
            source = source.to_string_lossy()
        ));
    }
    module.push_str("];\n");
    fs::write(out.join("embedded_content.rs"), module)?;
    Ok(())
}

fn build_doc_tree(
    name: &str,
    source_root: &Path,
    generated_root: &Path,
    style_path: &Path,
    entries: &mut Vec<(String, &'static str, PathBuf)>,
) -> Result<(), Box<dyn std::error::Error>> {
    let output_root = generated_root.join(name);
    fs::create_dir_all(&output_root)?;
    let mut directory_pages: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();

    for item in WalkDir::new(source_root) {
        let item = item?;
        if !item.file_type().is_file()
            || item.path().extension().and_then(|value| value.to_str()) != Some("md")
        {
            continue;
        }
        let relative = item.path().strip_prefix(source_root)?;
        let output_relative = markdown_output_path(relative);
        let output_path = output_root.join(&output_relative);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let markdown = fs::read_to_string(item.path())?;
        let title = first_heading(&markdown).unwrap_or_else(|| {
            relative
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        });
        let body = render_markdown(&markdown);
        let style_href = relative_style_href(&output_relative);
        fs::write(&output_path, html_page(&title, &style_href, &body))?;
        entries.push((
            format!("{name}/{}", slash_path(&output_relative)),
            "text/html; charset=utf-8",
            fs::canonicalize(&output_path)?,
        ));

        let parent = output_relative.parent().unwrap_or_else(|| Path::new(""));
        directory_pages
            .entry(parent.to_path_buf())
            .or_default()
            .insert(
                output_relative
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            );
        for ancestor in parent.ancestors() {
            if ancestor == parent {
                continue;
            }
            if let Ok(child) = parent.strip_prefix(ancestor) {
                if let Some(component) = child.components().next() {
                    directory_pages
                        .entry(ancestor.to_path_buf())
                        .or_default()
                        .insert(format!("{}/", component.as_os_str().to_string_lossy()));
                }
            }
        }
    }

    for (directory, children) in directory_pages {
        let index_relative = directory.join("index.html");
        let index_path = output_root.join(&index_relative);
        if index_path.exists() {
            continue;
        }
        fs::create_dir_all(index_path.parent().unwrap())?;
        let mut body = String::from("<h1>Documentation</h1><ul>");
        for child in children {
            let href = if child.ends_with('/') {
                format!("{child}index.html")
            } else {
                child.clone()
            };
            body.push_str(&format!(
                "<li><a href=\"{}\">{}</a></li>",
                escape_html(&href),
                escape_html(&child)
            ));
        }
        body.push_str("</ul>");
        fs::write(
            &index_path,
            html_page(
                "Documentation",
                &relative_style_href(&index_relative),
                &body,
            ),
        )?;
        entries.push((
            format!("{name}/{}", slash_path(&index_relative)),
            "text/html; charset=utf-8",
            fs::canonicalize(index_path)?,
        ));
    }

    entries.push((
        format!("{name}/style.css"),
        "text/css; charset=utf-8",
        fs::canonicalize(style_path)?,
    ));
    Ok(())
}

fn markdown_output_path(path: &Path) -> PathBuf {
    let mut output = path.to_path_buf();
    if path.file_name().and_then(|value| value.to_str()) == Some("README.md") {
        output.set_file_name("index.html");
    } else {
        output.set_extension("html");
    }
    output
}

fn render_markdown(markdown: &str) -> String {
    let parser = Parser::new_ext(markdown, Options::all()).map(|event| match event {
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => Event::Start(Tag::Link {
            link_type,
            dest_url: CowStr::Boxed(rewrite_link(&dest_url).into_boxed_str()),
            title,
            id,
        }),
        other => other,
    });
    let mut output = String::new();
    html::push_html(&mut output, parser);
    output
}

fn rewrite_link(link: &str) -> String {
    if link.starts_with("http:")
        || link.starts_with("https:")
        || link.starts_with("mailto:")
        || link.starts_with('#')
        || link.starts_with('/')
    {
        return link.to_owned();
    }
    let split = link.find(['#', '?']).unwrap_or(link.len());
    let (path, suffix) = link.split_at(split);
    let rewritten = if let Some(base) = path.strip_suffix("README.md") {
        format!("{base}index.html")
    } else if let Some(base) = path.strip_suffix(".md") {
        format!("{base}.html")
    } else if path.ends_with('/') {
        format!("{path}index.html")
    } else {
        path.to_owned()
    };
    format!("{rewritten}{suffix}")
}

fn first_heading(markdown: &str) -> Option<String> {
    markdown
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
}

fn relative_style_href(output: &Path) -> String {
    let depth = output
        .parent()
        .map_or(0, |parent| parent.components().count());
    format!("{}style.css", "../".repeat(depth))
}

fn html_page(title: &str, style_href: &str, body: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title><link rel=\"stylesheet\" href=\"{}\"></head><body>{}</body></html>",
        escape_html(title),
        escape_html(style_href),
        body
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn slash_path(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
