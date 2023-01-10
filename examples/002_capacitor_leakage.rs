use std::time::{Duration, Instant};
use vxi11::devices::sds1202x::SDS1202X;
use vxi11::devices::spd3303x::SPD3303X;
use vxi11::utils::LinearFitProblem;

const VOLTAGE: f32 = 2.7;
const I_FULL: f32 = 0.1;

const TDIV_SEC: f32 = 20.0;
const VDIV_VOLT: f32 = 1.0;

fn main() -> Result<(), &'static str> {

    let start_time = Instant::now();
    let mut runs_v0 = LinearFitProblem::default();
    let mut runs_vd = LinearFitProblem::default();

    // Channel 1 of the power supply is connected to a capacitor
    // Channel 1 of the oscilloscope is connected across the capacitor
    // There's no resistor.  This test is just designed to test the leakage
    // of charge stored in a capacitor over a long time

    let mut power = SPD3303X::new("25.0.0.1").unwrap();
    let mut oscilloscope = SDS1202X::new("25.0.0.2").unwrap();

    // Charge the capacitor
    power.set_voltage(1, VOLTAGE).unwrap();
    power.set_current(1, 3.0).unwrap();

    println!("Enable output on power supply");
    while power.measure_current(1).unwrap() < I_FULL {
        power.enable_output(1).unwrap();
        std::thread::sleep(Duration::from_secs(10));
    }

    println!("Charging capacitor ...");
    while power.measure_current(1).unwrap() > I_FULL {
        std::thread::sleep(Duration::from_secs(1));
    }

    // Turn off the power
    println!("Disable output on power supply and discharge capacitor");
    power.disable_output(1).unwrap();

    loop {

        // Capture the voltage profile as the capacitor discharges
        // across the resistor
        oscilloscope.set_time_division(TDIV_SEC).unwrap();
        oscilloscope.set_voltage_div(1, VDIV_VOLT).unwrap();

        oscilloscope.arm_single().unwrap();
        std::thread::sleep(Duration::from_secs(2));

        oscilloscope.force_trigger().unwrap();
        std::thread::sleep(Duration::from_secs_f32(TDIV_SEC*14.1));

        let wf: Vec<(f32, f32)> = oscilloscope.transfer_waveform(1).unwrap();
        let problem = LinearFitProblem {
            points: wf.into_iter().map(|(t, v)| (t as f64, v as f64)).collect()
        };

        let fit = problem.solve()?;

        let v0 = fit.intercept;
        let vd = fit.slope;

        println!("V0:    {:.4} [V]", v0);
        println!("dV/dt: {:.1e} [V/sec]", vd);

        runs_v0.points.push((start_time.elapsed().as_secs_f64(), v0));
        runs_vd.points.push((start_time.elapsed().as_secs_f64(), vd));

        std::fs::write(
            "./ex002_v0.json",
            serde_json::to_string_pretty(&runs_v0.points).unwrap().as_bytes()
        ).unwrap();

        std::fs::write(
            "./ex002_vd.json",
            serde_json::to_string_pretty(&runs_vd.points).unwrap().as_bytes()
        ).unwrap();

        if runs_v0.points.len() > 5 {
            let fit_v0 = runs_v0.solve()?;
            let fit_vd = runs_vd.solve()?;

            println!("V0 trend: {:?}", fit_v0);
            println!("Vd trend: {:?}", fit_vd);
        }
    }

}