#[derive(clap::Parser)]
pub struct Config {
    #[clap(long, env)]
    pub database_url: String,

    /// The HMAC signing and verification key used for login tokens (JWTs).
    ///
    /// There is no required structure or format to this key as it's just fed into a hash function.
    /// In practice, it should be a long, random string that would be infeasible to brute-force.
    #[clap(long, env)]
    pub hmac_key: String,

    #[clap(long, env)]
    pub upload_dir: String,

    #[clap(long, env)]
    pub config_file: String,

    #[clap(long, env)]
    pub rabbitmq_url: String,

    #[clap(long, env)]
    pub unstructured_url: String,

    #[clap(long, env)]
    pub es_url: String,
}
