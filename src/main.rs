use rand::SeedableRng;
use std::io::Write;
use std::f64::consts::FRAC_PI_2;
use std::f64::consts::FRAC_2_PI;
use byteorder::{BE,LE};
use byteorder::WriteBytesExt;

#[derive(strum_macros::EnumString)]
#[strum(ascii_case_insensitive)]
enum BinaryFormat {
    F32BE,
    F32LE,
    F64BE,
    F64LE,
    U8,
    U16BE,
    U16LE,
    U32BE,
    U32LE,
    U64BE,
    U64LE,
    S8,
    S16BE,
    S16LE,
    S32BE,
    S32LE,
    S64BE,
    S64LE,
}

/// Command-line tool to generate samples of various random distributions.
/// Note that more single-value distributions that are mentioned in https://docs.rs/statrs/0.15.0/statrs/distribution/index.html are easy to add to the tool.
#[derive(argh::FromArgs)]
struct Opts {
    /// number of digits after decimal to print
    #[argh(option,default="10",short='p')]
    precision: usize,

    /// add value of each sample to accumulator, outputting a random walk instead of individual samples
    #[argh(switch,short='C')]
    cumulative: bool,

    /// use specified seed instead for PRNG
    #[argh(option,short='S')]
    seed: Option<u64>,

    /// output as binary numbers of specified format instead of text.
    /// Valid formats are f{{32,64}}{{be,le}}, {{u,s}}8, {{u,s}}{{16,32,64}}{{le,be}}.
    /// Out of range values are clamped to valid ranges
    #[argh(option,short='b')]
    binary_format: Option<BinaryFormat>,

    /// number of sampels to generate, instead of an infinite stream
    #[argh(option,short='n')]
    num_samples: Option<u64>,

    /// exponentiate (e^x) each sample, producing log-normal instead of normal distribution, log-Cauchy instead of Cauchy, etc.
    #[argh(switch,short='e')]
    exponentiate: bool,

    /// discard samples that are below the specified value
    #[argh(option,short='L')]
    discard_below: Option<f64>,

    /// discard samples that are above the specified value
    #[argh(option,short='H')]
    discard_above: Option<f64>,

    #[argh(subcommand)]
    distribution : Distributions,
}

#[derive(argh::FromArgs)]
#[argh(subcommand)]
enum Distributions {
    Uniform(Uniform),
    Normal(Normal),
    Cauchy(Cauchy),
    Triangular(Triangular),
    StudentsT(StudentsT),
    Stable(Stable),
    Empirical(Empirical),
    Categorical(Categorical),
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
struct Uniform {
    /// include specified maximum value as possible candidate for generation
    #[argh(switch)]
    right_inclusive: bool,

    #[argh(positional)]
    min: f64,

    #[argh(positional)]
    max: f64,
}

/// Normal, Gaussian distribution
#[derive(argh::FromArgs)]
#[argh(subcommand, name="normal")]
struct Normal {
    #[argh(positional)]
    mean: f64,

    #[argh(positional)]
    std_dev: f64,
}

/// Cauchy, Lorentz distribution - fat-tailed and continuous
#[derive(argh::FromArgs)]
#[argh(subcommand, name="cauchy")]
struct Cauchy {
    #[argh(positional)]
    location: f64,

    #[argh(positional)]
    scale: f64,
}


/// Triangular distribution - continuous
#[derive(argh::FromArgs)]
#[argh(subcommand, name="triangular")]
struct Triangular {
    #[argh(positional)]
    min: f64,


    #[argh(positional)]
    mode: f64,

    #[argh(positional)]
    max: f64,
}


/// Student-T distribution
#[derive(argh::FromArgs)]
#[argh(subcommand, name="studentt")]
struct StudentsT {
    #[argh(positional)]
    location: f64,


    #[argh(positional)]
    scale: f64,

    #[argh(positional)]
    freedom: f64,
}

/// General case of stable continuous distribution, generated by CMS method.
/// Note that version 0.1 of this program used nonstandard distribution when alpha was not 1.0.
#[derive(argh::FromArgs)]
#[argh(subcommand, name="stable")]
struct Stable {
    #[argh(positional)]
    location: f64,


    #[argh(positional)]
    scale: f64,

    #[argh(positional)]
    alpha: f64,

    #[argh(positional)]
    beta: f64,
}


/// Discrete distribution that just endlessly randomly selects one of specified values
#[derive(argh::FromArgs)]
#[argh(subcommand, name="empirical")]
struct Empirical {
    #[argh(positional)]
    data_points: Vec<f64>,
}


/// Discrete distribution that generates values according to specified probabilities 
#[derive(argh::FromArgs)]
#[argh(subcommand, name="categorical")]
struct Categorical {
    #[argh(positional)]
    probabilities: Vec<f64>,
}

struct StableAlphaNotOne {
    location: f64,
    alpha: f64,
    u_dist: rand::distributions::Uniform<f64>,
    w_dist: statrs::distribution::Exp,
    calc_scale: f64,
    xi: f64,
    alpha_inv: f64,
    alpha2: f64,
}

impl StableAlphaNotOne {
    pub fn new(location: f64, scale: f64, alpha: f64, beta: f64) -> Self {
        let zeta = -beta * (FRAC_PI_2*alpha).tan();
        Self {
            location,
            alpha,
            u_dist: rand::distributions::Uniform::new(-FRAC_PI_2 + 3.0*f64::EPSILON, FRAC_PI_2 ),
            w_dist: statrs::distribution::Exp::new(1.0).unwrap(),
            calc_scale: (zeta*zeta+1.0).powf(0.5/alpha)*scale,
            xi: (-zeta).atan() / alpha,
            alpha_inv: 1.0/alpha,
            alpha2: (1.0 - alpha)/alpha,
        }
    }
}

/// Implementation is based on https://en.wikipedia.org/w/index.php?title=Stable_distribution&oldid=1025369901
impl DistributionObject for StableAlphaNotOne {
    fn sample(&self, rng: &mut rand::rngs::SmallRng) -> f64 {
        let u = self.u_dist.sample(rng);
        let w = self.w_dist.sample(rng);
        let num1 = (self.alpha*(u + self.xi)).sin();
        let den1 = u.cos().powf(self.alpha_inv);
        let num2 = (u - self.alpha * (u + self.xi)).cos() / w;
        self.location + self.calc_scale * num1 / den1 * (num2).powf(self.alpha2)
    }
}

struct StableAlphaOne {
    location: f64,
    beta: f64,
    u_dist: rand::distributions::Uniform<f64>,
    w_dist: statrs::distribution::Exp,
    calc_scale: f64,
}

impl StableAlphaOne {
    pub fn new(location: f64, scale: f64, beta: f64) -> Self {
        //scale *= std::f64::consts::FRAC_1_SQRT_2;
        Self {
            location: location + FRAC_2_PI * beta * scale * scale.ln(),
            beta,
            u_dist: rand::distributions::Uniform::new(-FRAC_PI_2 + 3.0*f64::EPSILON, FRAC_PI_2 ),
            w_dist: statrs::distribution::Exp::new(1.0).unwrap(),
            calc_scale: scale * FRAC_2_PI,
        }
    }
}

/// Implementation is based on https://en.wikipedia.org/w/index.php?title=Stable_distribution&oldid=1025369901
impl DistributionObject for StableAlphaOne {
    fn sample(&self, rng: &mut rand::rngs::SmallRng) -> f64 {
        let u = self.u_dist.sample(rng);
        let w = self.w_dist.sample(rng);
        self.location + self.calc_scale * ( (FRAC_PI_2 + self.beta * u) * u.tan() - self.beta * ( (FRAC_PI_2 * w * u.cos())/(FRAC_PI_2 + self.beta*u) ).ln() )
    }
}


fn main() -> anyhow::Result<()> {
    let opts : Opts = argh::from_env();

    let so = std::io::stdout();
    let so = so.lock();
    let mut so = std::io::BufWriter::with_capacity(32768, so);

    let d : Box<dyn DistributionObject>;
    d = match opts.distribution {
        Distributions::Uniform(Uniform { right_inclusive, min, max }) => {
            if max <= min {
                anyhow::bail!("Invalid distribution parameters");
            }
            if right_inclusive {
                Box::new(rand::distributions::Uniform::new_inclusive(min, max))
            } else {
                Box::new(rand::distributions::Uniform::new(min, max))
            }
        }
        Distributions::Normal(Normal { mean, std_dev }) => Box::new(statrs::distribution::Normal::new(mean, std_dev)?),
        Distributions::Cauchy(Cauchy { location, scale }) => Box::new(statrs::distribution::Cauchy::new(location, scale)?),
        Distributions::Triangular(Triangular { min, mode, max }) => Box::new(statrs::distribution::Triangular::new(min,max,mode)?),
        Distributions::StudentsT(StudentsT { location, scale, freedom }) =>  Box::new(statrs::distribution::StudentsT::new(location,scale,freedom)?),
        Distributions::Stable(Stable { location, scale, alpha, beta }) => {
            if alpha < 0.0 || alpha > 2.0 {
                anyhow::bail!("alpha must be between 0 and 2");
            }
            if beta < -1.0 || beta > 1.0 {
                anyhow::bail!("beta must be between -1 and 1");
            }
            if alpha > 0.999 && alpha < 1.001 {
                Box::new(StableAlphaOne::new(location,scale,beta))
            } else {
                Box::new(StableAlphaNotOne::new(location,scale,alpha,beta))
            }
        }
        Distributions::Empirical(Empirical { data_points }) => Box::new(statrs::distribution::Empirical::from_vec(data_points)),
        Distributions::Categorical(Categorical { probabilities }) =>  Box::new(statrs::distribution::Categorical::new(&probabilities)?),
    };
    
    let mut r = if let Some(s) = opts.seed {
        rand::rngs::SmallRng::seed_from_u64(s)
    } else {
        rand::rngs::SmallRng::from_entropy()
    };

    let mut c : f64 = 0.0;
    let mut counter : u64 = 0;
    loop {
        if let Some(limit) = opts.num_samples {
            if counter >= limit {
                break;
            }
        }
        let mut x = d.sample(&mut r);

        if opts.exponentiate { x = x.exp(); }

        if let Some(limit) = opts.discard_below {
            if x < limit {
                continue;
            }
        }
        if let Some(limit) = opts.discard_above {
            if x > limit {
                continue;
            }
        }

        c += x;
        match opts.binary_format {
            None => writeln!(so, "{:.*}", opts.precision, c)?,
            Some(BinaryFormat::F32LE) => so.write_f32::<LE>(c as f32)?,
            Some(BinaryFormat::F32BE) => so.write_f32::<BE>(c as f32)?,
            Some(BinaryFormat::F64LE) => so.write_f64::<LE>(c)?,
            Some(BinaryFormat::F64BE) => so.write_f64::<BE>(c)?,
            Some(BinaryFormat::S8) => so.write_i8(c as i8)?,
            Some(BinaryFormat::U8) => so.write_u8(c as u8)?,
            Some(BinaryFormat::S16LE) => so.write_i16::<LE>(c as i16)?,
            Some(BinaryFormat::S16BE) => so.write_i16::<BE>(c as i16)?,
            Some(BinaryFormat::U16LE) => so.write_u16::<LE>(c as u16)?,
            Some(BinaryFormat::U16BE) => so.write_u16::<BE>(c as u16)?,
            Some(BinaryFormat::S32LE) => so.write_i32::<LE>(c as i32)?,
            Some(BinaryFormat::S32BE) => so.write_i32::<BE>(c as i32)?,
            Some(BinaryFormat::U32LE) => so.write_u32::<LE>(c as u32)?,
            Some(BinaryFormat::U32BE) => so.write_u32::<BE>(c as u32)?,
            Some(BinaryFormat::S64LE) => so.write_i64::<LE>(c as i64)?,
            Some(BinaryFormat::S64BE) => so.write_i64::<BE>(c as i64)?,
            Some(BinaryFormat::U64LE) => so.write_u64::<LE>(c as u64)?,
            Some(BinaryFormat::U64BE) => so.write_u64::<BE>(c as u64)?,
        }
        
        if ! opts.cumulative { c = 0.0; }
        counter = counter.wrapping_add(1);
    }
    Ok(())
}
