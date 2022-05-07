
// Perform a 1-D least squared linear fit

#[derive(Default)]
pub struct LinearFitProblem {
    pub points: Vec<(f64, f64)>
}

#[derive(Debug)]
pub struct LinearFit {
    pub slope: f64,
    pub intercept: f64,
}

impl LinearFitProblem {

    pub fn solve(&self) -> Result<LinearFit, &'static str> {
        let n = self.points.len() as f64;
        let xx: f64 = self.points.iter().map(|(x, _)| *x * *x).sum();
        let xy: f64 = self.points.iter().map(|(x, y)| *x * *y).sum();
        let x: f64 = self.points.iter().map(|(x, _)| *x).sum();
        let y: f64 = self.points.iter().map(|(_, y)| *y).sum();

        let denom: f64 = n*xx - x.powi(2);
        if denom == 0.0 {
            Err("Singular least squares problem")
        } else {
            let det: f64 = 1.0 / denom;
            Ok(LinearFit {
                slope:     det*( n*xy - x*y),
                intercept: det*(-x*xy + y*xx)
            })
        }
    }

}
