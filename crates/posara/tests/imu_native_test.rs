#![cfg(feature = "compiler")]

use posara::backend::Backend;
use posara::runner::{compile_abe, Stepper};
use posara::Host;
use posara_core::drivers::Imu;

struct Fixed([i32; 3]);

impl Imu for Fixed {
    fn accel(&mut self) -> [i32; 3] { self.0 }
}

fn read_axis(tag: &str, backend: Backend, axis: i64) -> i64 {
    let root = std::env::temp_dir();
    let p = root.join(format!("posara_imu_{tag}_{axis}.abe"));
    std::fs::write(&p, format!("@cart\nfn main() -> <frame, IO> Unit {{\n  frame.present();\n  halt(imu_accel({axis}) + 100000);\n  loop {{ frame.present() }}\n}}\n")).unwrap();
    let host = Host::with_backend(root, backend, true, true).unwrap();
    let r = compile_abe(&p, &host).unwrap_or_else(|e| panic!("compile:\n{e}"));
    let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();
    step.frame().unwrap();
    std::fs::remove_file(&p).ok();
    step.exit_code() - 100000
}

#[test]
fn imu_accel_reads_raw_axes() {
    let root = std::env::temp_dir();
    for (axis, want) in [(0, 100), (1, -200), (2, 1000)] {
        let b = Backend::null(root.clone()).imu(Box::new(Fixed([100, -200, 1000])));
        assert_eq!(read_axis("fixed", b, axis), want, "axis {axis}");
    }
}

#[test]
fn imu_accel_is_zero_without_sensor() {
    for axis in 0..3 {
        assert_eq!(read_axis("null", Backend::null(std::env::temp_dir()), axis), 0);
    }
}
