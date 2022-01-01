use clap::ArgMatches;
use anyhow::Result;


pub async fn client_mode(matches:ArgMatches<'static>) -> Result<()> {
    let mode = matches.value_of("key").unwrap_or("PUNHwXvbUpz2VxgtFzOYzas/vkJubvr0SEmYLFudTZPZGjAh+WD24Kw/Mg/uO75nq5Kdaa9AnyjGxd22VnmSVFfFEU72JdKVGStBTkrs4PrYCxyZ9qBnhMEHPtzgztD5");
    let iv = matches.value_of("iv").unwrap_or("12345678");
    
    Ok(())
}