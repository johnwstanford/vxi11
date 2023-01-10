use std::time::{Duration, Instant};
use vxi11::devices::sds1202x::SDS1202X;
use vxi11::devices::spd3303x::SPD3303X;
use vxi11::utils::LinearFitProblem;

const R_OHMS: f32 = 10.0;
const VOLTAGE: f32 = 2.7;
const I_FULL: f32 = VOLTAGE / R_OHMS;
const MAX_EXPECTED_C: f64 = 1.0;

const TDIV_SEC: f32 = 0.01;
const VDIV_VOLT: f32 = 1.0;

fn main() -> Result<(), &'static str> {

    let start_time = Instant::now();
    let mut runs_c = LinearFitProblem::default();

    // Channel 1 of the power supply is connected to a resistor and capacitor connected
    // in parallel
    // Channel 1 of the oscilloscope is connected across the resistor and capacitor

    let mut power = SPD3303X::new("25.0.0.1").unwrap();
    let mut oscilloscope = SDS1202X::new("25.0.0.2").unwrap();

    // Charge the capacitor
    power.set_voltage(1, VOLTAGE).unwrap();
    power.set_current(1, 3.0).unwrap();

    loop {
        println!("Enable output on power supply");
        while power.measure_current(1).unwrap() < I_FULL {
            power.enable_output(1).unwrap();
            std::thread::sleep(Duration::from_secs(1));
        }

        println!("Charging capacitor ...");
        while power.measure_current(1).unwrap() > I_FULL*4.0 {
            std::thread::sleep(Duration::from_secs(1));
        }

        // Capture the voltage profile as the capacitor discharges
        // across the resistor
        oscilloscope.set_time_division(TDIV_SEC).unwrap();
        oscilloscope.set_voltage_div(1, VDIV_VOLT).unwrap();

        oscilloscope.arm().unwrap();
        std::thread::sleep(Duration::from_secs(1));

        // Turn off the power
        println!("Disable output on power supply and discharge capacitor");
        power.disable_output(1).unwrap();
        std::thread::sleep(Duration::from_secs(1));

        let wf: Vec<(f32, f32)> = oscilloscope.transfer_waveform(1).unwrap();
        let max_millivolts: usize = wf.iter().map(|(_, v)| (*v * 1000.0) as usize).max().unwrap();
        let min_millivolts: usize = wf.iter().map(|(_, v)| (*v * 1000.0) as usize).min().unwrap();
        let rng_millivolts: usize = max_millivolts - min_millivolts;
        let keep_min_v: f32 = (min_millivolts as f32 + (rng_millivolts as f32 * 0.1)) / 1000.0;
        let keep_max_v: f32 = (min_millivolts as f32 + (rng_millivolts as f32 * 0.9)) / 1000.0;
        let problem = LinearFitProblem {
            points: wf.into_iter()
                .filter(|(_, v)| keep_min_v < *v && *v < keep_max_v)
                .map(|(t, v)| (t as f64, v.ln() as f64)).collect()
        };

        let fit = problem.solve()?;

        let c = -1.0/(fit.slope * (R_OHMS as f64));

        if c < MAX_EXPECTED_C {

            println!("C: {:.1e} [F]", c);

            runs_c.points.push((start_time.elapsed().as_secs_f64(), c));

            std::fs::write(
                "./ex001_c.json",
                serde_json::to_string_pretty(&runs_c.points).unwrap().as_bytes()
            ).unwrap();

            let sum_c: f64 = runs_c.points.iter().map(|(_, x)| *x).sum();
            let avg_c: f64 = sum_c / (runs_c.points.len() as f64);
            let ssq_c: f64 = runs_c.points.iter().map(|(_, x)| (*x - avg_c).powi(2)).sum();
            let var_c: f64 = ssq_c / (runs_c.points.len() as f64);
            println!("Capacitance: {:.1} +/- {:.2} [uF], N={}", avg_c*1.0e6, var_c.sqrt()*1.0e6, runs_c.points.len());

        }

    }

}