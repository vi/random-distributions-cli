use rand::SeedableRng;
use std::io::Write;


/// Command-line tool to generate samples of various random distributions
#[derive(argh::FromArgs)]
struct Opts {
    /// number of digits after decimal to print
    #[argh(option,default="10",short='p')]
    precision: usize,

    #[argh(subcommand)]
    distribution : Distributions,
}

#[derive(argh::FromArgs)]
#[argh(subcommand)]
enum Distributions {
    Uniform(UniformD),
}

trait DistributionObject {
    fn sample(&self, rng: &mut rand::rngs::SmallRng) -> f64;
}
impl<T: rand::distributions::Distribution<f64>> DistributionObject for T {
    fn sample(&self, rng: &mut rand::rngs::SmallRng) -> f64 {
        rand::distributions::Distribution::sample(self, rng)
    }
}

/// Uniform distribution
#[derive(argh::FromArgs)]
#[argh(subcommand, name="uniform")]
struct UniformD {
    /// include specified maximum value as possible candidate for generation
    #[argh(switch)]
    right_inclusive: bool,

    #[argh(positional)]
    min: f64,

    #[argh(positional)]
    max: f64,
}

fn main() -> anyhow::Result<()> {
    let opts : Opts = argh::from_env();

    let so = std::io::stdout();
    let so = so.lock();
    let mut so = std::io::BufWriter::with_capacity(32768, so);

    let d : Box<dyn DistributionObject>;
    d = match opts.distribution {
        Distributions::Uniform(UniformD { right_inclusive, min, max }) => {
            if right_inclusive {
                Box::new(rand::distributions::Uniform::new_inclusive(min, max))
            } else {
                Box::new(rand::distributions::Uniform::new(min, max))
            }
        }
    };
    
    let mut r = rand::rngs::SmallRng::from_entropy();

    loop {
        let x = d.sample(&mut r);
        writeln!(so, "{:.*}", opts.precision, x)?;
    }
}
