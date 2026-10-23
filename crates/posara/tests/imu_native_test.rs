#![cfg(feature = "compiler")]

use posara::backend::Backend;
use posara::runner::Stepper;
use posara::Host;
use posara_core::drivers::Imu;

struct Fixed([i32; 3]);

impl Imu for Fixed {
    fn accel(&mut self) -> [i32; 3] { self.0 }
}

#[test]
fn imu_accel_reads_axes_or_zero_without_sensor() {
    let root = std::env::temp_dir();
    let src = "@cart\nfn main() -> <frame, IO> Unit {\n  frame.present();\n  let x = imu_accel(0) + 300;\n  let y = imu_accel(1) + 300;\n  let z = imu_accel(2) + 300;\n  halt(x + y * 1024 + z * 1048576);\n  loop { frame.present() }\n}\n";
    let run = |backend: Backend| {
        let host = Host::with_backend(root.clone(), backend, true, true).unwrap();
        let r = posara::runner::compile_source(src, &host).unwrap();
        let mut step = Stepper::start_named(r.module, r.static_names, r.fn_names, &host).unwrap();
        step.frame().unwrap();
        let v = step.exit_code();
        [(v & 0x3FF) - 300, ((v >> 10) & 0x3FF) - 300, ((v >> 20) & 0x3FF) - 300]
    };
    assert_eq!(run(Backend::null(root.clone()).imu(Box::new(Fixed([100, -200, 500])))), [100, -200, 500]);
    assert_eq!(run(Backend::null(root.clone())), [0, 0, 0]);
}
