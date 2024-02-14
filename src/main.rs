use std::{
    collections::HashMap,
    fs::{self, File},
    path::PathBuf,
    process::{Command, Stdio},
    result::Result::Ok,
    time::Duration,
};

use anyhow::{anyhow, Result};
use clap::Parser;
use colored::Colorize;
use handlebars::Handlebars;
use indicatif::{ProgressBar, ProgressStyle};
use opener::open;
use serde_json::Value;

#[derive(Debug, Parser)]
struct Arguments {
    #[arg(help = "Path to conan package")]
    src: PathBuf,

    #[arg(long, help = "Path to output folder")]
    out: Option<PathBuf>,

    #[arg(long, help = "Open generated documentation")]
    open: bool,
}

fn with_progress_bar<F, T>(msg: String, f: F) -> Result<T>
where
    F: FnOnce() -> Result<(String, T)>,
{
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner().template("{spinner} {wide_msg} [{elapsed_precise}]")?,
    );

    pb.enable_steady_tick(Duration::from_millis(50));
    pb.set_message(format!("{}", msg.yellow()));
    let res = f();
    match res {
        Ok((msg, val)) => {
            pb.finish_with_message(format!("{}", msg.green()));
            Ok(val)
        }
        Err(e) => {
            pb.finish_with_message(format!("Error: {}", e.to_string().red()));
            Err(e)
        }
    }
}

fn gather_sources(src_pkg: &str) -> Result<(String, Vec<String>)> {
    let info_output_raw = Command::new("conan")
        .args(["info", src_pkg, "--paths", "--json"])
        .output()?
        .stdout;

    let info_output_raw_str = String::from_utf8(info_output_raw)?;
    let temp = info_output_raw_str.split('\n').collect::<Vec<&str>>();
    let info_json_raw = temp.last().ok_or(anyhow!("Failed to get package paths"))?;
    let info_json_obj: Vec<Value> = serde_json::from_str(info_json_raw)?;
    let mut source_folders = Vec::new();
    for obj in info_json_obj {
        match obj.get("package_folder") {
            Some(val) => {
                if let Some(s) = val.as_str() {
                    source_folders.push(s.to_string());
                }
            }
            None => continue,
        }
    }

    source_folders.push(format!("{}/sources", src_pkg));
    Ok((
        format!("Found {} source locations", source_folders.len()),
        source_folders,
    ))
}

fn conan_install(src_pkg: &str) -> Result<(String, ())> {
    let install_folder = format!("{}/.conan", src_pkg );
    Command::new("cdt")
        .args(["conan", "install", src_pkg, "-pr", "default", "-if", install_folder.as_str() ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    Ok((String::from("Finished conan install"), ()))
}

fn inspect(src_pkg: &str) -> Result<(String, String, Vec<String>)> {
    let name_bytes = Command::new("conan")
        .args(["inspect", src_pkg, "--raw", "name"])
        .output()?
        .stdout;

    let version_bytes = Command::new("conan")
        .args(["inspect", src_pkg, "--raw", "version"])
        .output()?
        .stdout;

    let requires_bytes = Command::new("conan")
        .args(["inspect", src_pkg, "--raw", "requires"])
        .output()?
        .stdout;

    let name = String::from_utf8(name_bytes)?;
    let version = String::from_utf8(version_bytes)?;
    let requires = String::from_utf8(requires_bytes)?
        .split(',')
        .map(|s| s.trim_start_matches('['))
        .map(|s| s.trim_end_matches(']'))
        .map(|s| s.trim().replace('\'', ""))
        .collect::<Vec<String>>();

    Ok((name, version, requires))
}

fn generate_doxyfile(
    name: &String,
    version: &String,
    sources_str: &String,
    output_str: &String,
) -> Result<(String, PathBuf)> {
    let mut handlebars = Handlebars::new();
    let mut handlebar_data = HashMap::new();
    handlebar_data.insert("name", name);
    handlebar_data.insert("version", version);
    handlebar_data.insert("sources", sources_str);
    handlebar_data.insert("output", output_str);

    let doxy_folder_out = format!("{}/.doxy", output_str);
    let doxy_file_out = format!("{}/DoxyFile", &doxy_folder_out);

    fs::create_dir_all(&doxy_folder_out).expect("Unable to create directory");
    let mut output_file = File::create(&doxy_file_out)?;

    handlebars.register_template_file("doxyfile", "./template/DoxyFile.hbs")?;

    handlebars.render_to_write("doxyfile", &handlebar_data, &mut output_file)?;
    Ok((
        String::from("Generated DoxyFile"),
        PathBuf::from(doxy_file_out),
    ))
}

fn main() -> Result<()> {
    let args = Arguments::parse();

    if let Some(src_pkg) = args.src.to_str() {
        // conan inspect
        let (name, version, requires) = inspect(src_pkg)?;
        println!(
            "Generating documentation for {}/{} with \n {:#?}",
            name.green(),
            version.green(),
            requires
        );

        // conan install
        with_progress_bar("[1/5] Fetching packages...".to_string(), || {
            conan_install(src_pkg)
        })?;

        // conan info
        let source_folders = with_progress_bar("[2/5] Gathering Sources...".to_string(), || {
            gather_sources(src_pkg)
        })?;

        // output path
        let output_str = with_progress_bar("[3/5] Resolving Output...".to_string(), || {
            let output_default =
                PathBuf::from(format!("{}/build/docs/{}_{}", src_pkg, name, version));
            let output_str = args
                .out
                .unwrap_or(output_default)
                .to_str()
                .ok_or_else(|| anyhow!("Failed to convert PathBuf to str"))?
                .to_string();
            Ok((format!("Output location is {}", output_str), output_str))
        })?;

        // Generate DoxyFile
        let doxy_file_out = with_progress_bar("[4/5] Generating Doxyfile...".to_string(), || {
            generate_doxyfile(&name, &version, &source_folders.join(" "), &output_str)
        })?;

        // Doxygen generate
        let status = with_progress_bar("[5/5] Running Doxygen...".to_string(), || {
            let status = Command::new("doxygen")
                .args([
                    &doxy_file_out
                        .to_str()
                        .ok_or(anyhow!("outpath could not be resolved"))?,
                    "-l",
                    "./template/Layout.xml"
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .expect("Failed to execute command");

            Ok((String::from("Finished Doxygen Generate"), status))
        })?;

        // open if success
        if status.success() {
            let path_to_html =
                PathBuf::from(format!("{}/html/index.html", &output_str)).canonicalize()?;
            let html_os_str = path_to_html.as_os_str().to_owned();
            let html = html_os_str.to_str().ok_or(anyhow!(" "))?;
            println!("\n Success: Docs can be found at {}", html.green());

            if args.open {
                match open(html) {
                    Ok(()) => println!("Opened '{}' successfully.", html),
                    Err(err) => eprintln!("An error occurred when opening '{}': {}", html, err),
                }
            }
        } else {
            return Err(anyhow!(
                "Failed to generate docs. Please ensure doxygen is available in PATH."
            ));
        }
    }

    Ok(())
}
