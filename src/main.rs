#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;
extern crate chrono;

use std::{thread, time};
use std::io::Write;
use std::fs;
use std::fs::File;
use std::path::Path;
use chrono::Local;
use std::process::Command;

const CHECK_INTERVAL: u32 = 5;
const WORK_TIME: u32 = 25 * 60;
const SHORT_REST_TIME: u32 = 5 * 60;
const LONG_REST_TIME: u32 = 15 * 60;
const DATA_PATH: &str = "./data/day_activity.json";


#[derive(Serialize, Deserialize, Debug, Default)]
struct DayActivity {
    is_working: bool,  // either break or work
    is_long_break: bool,  // either a long break or not
    break_time: u32,  // either 5 or 15 minutes related to is_long_break
    activities_count: u32,  // increase when 25 minutes elapsed
    breaks_count: u32,  // increase when 5 minutes of idle elapsed
    current_activity_time: u32,  // current user activity seconds
    current_idle_time: u32,  // current user idle seconds
    date: String,  // today date
    last_updated_at: String,  // last updated statistic
}


impl DayActivity {
    fn initial() -> DayActivity {
        let date = Local::now();
        DayActivity {
            is_working: false,
            is_long_break: false,
            break_time: 5,
            activities_count: 0,
            breaks_count: 0,
            current_activity_time: 0,
            current_idle_time: 0,
            date: date.format("%Y-%m-%dT%H:%M:%S").to_string(),
            last_updated_at: date.format("%Y-%m-%dT%H:%M:%S").to_string(),
        }
    }
}


fn shell(cmd: &str) -> String {
    let output = Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .output()
                .expect("failed to execute process");

    if !output.status.success() {
        panic!("the following command exited with error:\n{:?}", String::from_utf8_lossy(&output.stderr).to_string());
    } else {
        // remove \n
        let mut result = String::from_utf8_lossy(&output.stdout).to_string();
        if let Some('\n') = result.chars().next_back() {
            result.pop();
        }
        result
    }
}


#[allow(dead_code)]
fn shell_print(cmd: &str) {
    print!("{}", shell(cmd));
}


fn check_idle_time() -> u32 {
    let result = shell("echo $((`ioreg -c IOHIDSystem | sed -e '/HIDIdleTime/ !{ d' -e 't' -e '}' -e 's/.* = //g' -e 'q'` / 1000000000))");
    match result.parse::<u32>() {
        Ok(n) => n,
        Err(why) => {
            panic!("comand error {}", why);
        },
    }
}

fn notify(title: &str, subtitle: &str, message: &str) {
    println!("NOTIFY: {} {}", title, message);
    let cmd = format!(
        "terminal-notifier -sound default -title \"{title}\" -subtitle \"{subtitle}\" -message \"{message}\" -appIcon \"./timer.png\"",
        title=title,
        subtitle=subtitle,
        message=message,
    );
    shell(&cmd);
}

fn read_day_activity() -> DayActivity {
    if !Path::new(DATA_PATH).exists() {
        let day_activity = DayActivity::initial();
        write_day_activity(&day_activity);
        return day_activity
    }

    let data = fs::read_to_string(DATA_PATH).expect("Unable to read file");
    match serde_json::from_str(&data) {
        Ok(res) => res,
        Err(why) => panic!("{}", why),
    }
}

fn write_day_activity(day_activity: &DayActivity) {
    match serde_json::to_string(day_activity) {
        Ok(data) => {
            save_data_to_file(data.as_bytes());
        },
        Err(why) => panic!("{}", why),
    };
}


fn increase_idle<'a >(day_activity: &'a mut DayActivity, time: u32) {
    let date = Local::now();
    day_activity.last_updated_at = date.format("%Y-%m-%dT%H:%M:%S").to_string();
    day_activity.current_idle_time += time;
}

fn increase_activity<'a >(day_activity: &'a mut DayActivity, time: u32) {
    let date = Local::now();
    day_activity.last_updated_at = date.format("%Y-%m-%dT%H:%M:%S").to_string();
    day_activity.current_activity_time += time;
}


fn monitor_user_activity() {
    let mut day_activity = read_day_activity();
    let delay_time = time::Duration::from_millis(5000);
    day_activity.is_working = true;
    loop {
        let idle_time = check_idle_time();
        let is_idle = idle_time >= CHECK_INTERVAL;

        if is_idle {
            increase_idle(&mut day_activity, CHECK_INTERVAL);
        } else {
            increase_activity(&mut day_activity, CHECK_INTERVAL);
        }

        // Working less than 25 minutes
        if is_idle && !day_activity.is_working && day_activity.current_activity_time <= WORK_TIME {
            day_activity.is_working = true;
        }

        // Working more than 25 minutes
        if day_activity.current_activity_time >= WORK_TIME {
            if day_activity.is_working {
                if is_idle {
                    day_activity.is_working = false;
                    day_activity.current_activity_time = 0;
                    day_activity.activities_count += 1;
                } else {
                    // Each 4th break is long
                    day_activity.break_time = if day_activity.activities_count % 4 == 0 {
                        day_activity.is_long_break = true;
                        LONG_REST_TIME
                    } else {
                        day_activity.is_long_break = false;
                        SHORT_REST_TIME
                    };
                    notify("It's time to break!", "", format!("Take a {} minutes break", day_activity.break_time).as_str());
                }
            }
        }

        // Resting less than 5 or 25 minutes
        if day_activity.current_idle_time < day_activity.break_time {
            if !day_activity.is_working && !is_idle {
                // didn't have enough of rest
                notify("Your break is not finished yet!", "Take a break", "You started wroking too early");
            }
        }
        // Resting more than 5 or 25 minutes
        if day_activity.current_idle_time > day_activity.break_time && !day_activity.is_working {
            if is_idle {
                notify("It's time to work", "", "");
            } else {
                day_activity.is_working = true;
                day_activity.breaks_count += 1;
            }
        }

        write_day_activity(&day_activity);
        thread::sleep(delay_time);
    }
}

fn save_data_to_file(data: &[u8]) {
    let path = Path::new(DATA_PATH);
    let display = path.display();
    if !Path::new(DATA_PATH).exists() {
        match fs::create_dir("./data") {
            Ok(_) => println!("create dir"),
            Err(_why) => println!("can't create dir: {}", path.display()),
        };
    }

    let mut file = match File::create(&path) {
        Err(why) => panic!("couldn't create {}: {}", display, why),
        Ok(file) => file,
    };
    match file.write_all(data) {
        Err(why) => panic!("couldn't write to {}: {}", display, why),
        Ok(_) => {},
    }
}


fn main() {
    monitor_user_activity();
}
