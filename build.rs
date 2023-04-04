extern crate handlebars;
extern crate rustc_serialize;

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, create_dir_all, File};
use std::io::{Read, Write};
use std::path::Path;

use handlebars::{Context, Handlebars, Template};
use rustc_serialize::json::{Json, ToJson};

static UNITS_DIR: &'static str = "units";
static MAN_DIR: &'static str = "man";

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let units_out_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let output = Path::new(&*units_out_dir).join("out").join("build");

    create_dir_all(output.join("units")).unwrap();
    create_dir_all(output.join("man")).unwrap();

    let data = build_render_data();

    let mut config = File::create(out_dir.clone() + "/config.rs").unwrap();
    writeln!(
        config,
        "pub static USERS_CRONTAB_DIR: &str = {:?};",
        data["statedir"].as_string().unwrap()
    )
    .unwrap();
    writeln!(config, "pub static PACKAGE: &str = {:?};", data["package"].as_string().unwrap()).unwrap();
    writeln!(config, "pub static BIN_DIR: &str = {:?};", data["bindir"].as_string().unwrap()).unwrap();
    writeln!(config, "pub static LIB_DIR: &str = {:?};", data["libdir"].as_string().unwrap()).unwrap();

    let mut data = Json::Object(data);
    let schedules = get_required_schedules();

    for schedule in schedules.iter() {
        data.as_object_mut()
            .unwrap()
            .insert("schedule".to_owned(), Json::String(schedule.clone()));
        for schedule_unit in ["target", "timer", "service"].iter() {
            compile_template(
                format!("{}/cron-schedule.{}.in", UNITS_DIR, schedule_unit),
                output.join("units").join(format!("cron-{}.{}", schedule, schedule_unit)),
                &data,
            );
        }
    }

    data.as_object_mut().unwrap().insert("schedules".to_owned(), schedules.to_json());

    compile_templates(UNITS_DIR, output.join("units"), &data);
    compile_templates(MAN_DIR, output.join("man"), &data);
}

fn compile_template<S: AsRef<Path>, T: AsRef<Path>>(source_file: S, target_file: T, data: &Json) {
    println!(
        "compiling from template: {:?} -> {:?}...",
        source_file.as_ref(),
        target_file.as_ref()
    );

    let tmpl = File::open(source_file)
        .and_then(|mut file| {
            let mut buf = String::new();
            file.read_to_string(&mut buf).map(|_| Template::compile(&*buf).unwrap())
        })
        .unwrap();

    let mut handle = Handlebars::new();
    handle.register_template("default", tmpl);

    let ctx = Context::wraps(data);
    handle.renderw("default", &ctx, &mut File::create(target_file).unwrap()).unwrap();
}

fn compile_templates<P: AsRef<Path>>(source_dir: &str, output_dir: P, data: &Json) {
    for entry in fs::read_dir(source_dir).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        if name.ends_with(".in") && !name.starts_with("cron-schedule.") {
            let target = output_dir.as_ref().join(&name[..name.len() - 3]);
            compile_template(entry.path(), target, data);
        }
    }
}

fn build_render_data() -> BTreeMap<String, Json> {
    let mut ctx = BTreeMap::new();

    let prefix = env::var("PREFIX").unwrap_or_else(|_| "/usr/local".to_owned());
    let package = "systemd-cron";

    ctx.insert("package".to_owned(), Json::String(package.to_owned()));

    ctx.insert(
        "bindir".to_owned(),
        Json::String(env::var("BIN_DIR").unwrap_or_else(|_| prefix.clone() + "/bin")),
    );
    ctx.insert(
        "confdir".to_owned(),
        Json::String(env::var("CONF_DIR").unwrap_or_else(|_| prefix.clone() + "/etc")),
    );

    let datadir = env::var("DATA_DIR").unwrap_or_else(|_| prefix.clone() + "/share");
    let libdir = env::var("LIB_DIR").unwrap_or_else(|_| prefix.clone() + "/lib");

    ctx.insert(
        "mandir".to_owned(),
        Json::String(env::var("MAN_DIR").unwrap_or_else(|_| datadir.clone() + "/man")),
    );
    ctx.insert(
        "docdir".to_owned(),
        Json::String(env::var("DOC_DIR").unwrap_or_else(|_| datadir.clone() + "/doc/" + package)),
    );
    ctx.insert(
        "unitdir".to_owned(),
        Json::String(env::var("UNIT_DIR").unwrap_or_else(|_| libdir.clone() + "/systemd/system")),
    );

    ctx.insert("libdir".to_owned(), Json::String(libdir));
    ctx.insert("datadir".to_owned(), Json::String(datadir));
    ctx.insert("prefix".to_owned(), Json::String(prefix));

    ctx.insert(
        "statedir".to_owned(),
        Json::String(env::var("STATE_DIR").unwrap_or_else(|_| "/var/spool/cron".to_owned())),
    );

    ctx.insert(
        "runparts".to_owned(),
        Json::String(env::var("RUN_PARTS").unwrap_or_else(|_| "/usr/bin/run-parts".to_owned())),
    );

    ctx.insert("persistent".to_owned(), Json::Boolean(env::var("CARGO_FEATURE_PERSISTENT").is_ok()));

    ctx
}

fn get_required_schedules() -> Vec<String> {
    let features = [
        ("CARGO_FEATURE_SCHED_BOOT", "boot"),
        ("CARGO_FEATURE_SCHED_HOURLY", "hourly"),
        ("CARGO_FEATURE_SCHED_DAILY", "daily"),
        ("CARGO_FEATURE_SCHED_WEEKLY", "weekly"),
        ("CARGO_FEATURE_SCHED_MONTHLY", "monthly"),
        ("CARGO_FEATURE_SCHED_YEARLY", "yearly"),
        ("CARGO_FEATURE_SCHED_MINUTELY", "minutely"),
        ("CARGO_FEATURE_SCHED_QUARTERLY", "quarterly"),
        ("CARGO_FEATURE_SCHED_SEMI_ANNUALLY", "semi-annually"),
    ];

    features
        .iter()
        .filter(|&&(e, _)| env::var(e).is_ok())
        .map(|&(_, f)| f.to_owned())
        .collect::<Vec<_>>()
}
