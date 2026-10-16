use super::*;
pub(super) fn collect(c: &Context<'_>, rows: &mut Rows<'_>) -> Result<()> {
    let value = c.command_plist("pkgutil", &["--pkgs-plist"])?;
    let packages = value.as_array().ok_or(Status::Invalid)?;
    for name in packages {
        c.check()?;
        let name = name.as_string().ok_or(Status::Invalid)?;
        if name.starts_with('-') {
            return Err(Status::Invalid.into());
        }
        let data = c.command_plist("pkgutil", &["--pkg-info-plist", name])?;
        rows.push(decode(object(&data)?)?)?;
    }
    // A changing receipt set cannot claim a complete enumeration.
    if value != c.command_plist("pkgutil", &["--pkgs-plist"])? {
        return Err(Status::Invalid.into());
    }
    Ok(())
}
pub(super) fn decode(d: &plist::Dictionary) -> Result<Row> {
    let name = text(d, "pkgid")?;
    let volume = text(d, "volume")?;
    let location = text(d, "install-location")?;
    let path = Path::new(volume).join(location);
    let location = path.to_str().ok_or(Status::Invalid)?;
    let mut result = row(&[
        (
            "id",
            stable(&["pkg_receipt", name, volume, location]).into(),
        ),
        ("native_id", name.into()),
        ("name", name.into()),
        ("version", text(d, "pkg-version")?.into()),
        ("scope", "device".into()),
        ("owner", "".into()),
        ("origin", "pkg_receipt".into()),
        ("location", location.into()),
    ]);
    let timestamp = uint(d, "install-time")?;
    result.insert(
        "install_time".into(),
        i64::try_from(timestamp)
            .map_err(|_| Status::Invalid)?
            .into(),
    );
    Ok(result)
}
