#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionRelation {
    Compatible,
    UpdateHost,
    UpdatePlugin,
    Incompatible,
    Unknown,
}

fn parse(value: &str) -> Option<[u64; 3]> {
    let mut parts = value.split('.');
    let parsed: [u64; 3] = [
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ];
    if parts.next().is_some() || parsed.map(|part| part.to_string()).join(".") != value {
        return None;
    }
    Some(parsed)
}

pub fn compare(plugin: &str, host: &str) -> VersionRelation {
    let (Some(plugin), Some(host)) = (parse(plugin), parse(host)) else {
        return VersionRelation::Unknown;
    };
    if plugin[0] != host[0] {
        VersionRelation::Incompatible
    } else if plugin[1] > host[1] {
        VersionRelation::UpdateHost
    } else if plugin[1] < host[1] {
        VersionRelation::UpdatePlugin
    } else {
        VersionRelation::Compatible
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_patch_differences_are_silent() {
        assert_eq!(compare("1.2.9", "1.2.0"), VersionRelation::Compatible);
        assert_eq!(compare("1.3.0", "1.2.9"), VersionRelation::UpdateHost);
        assert_eq!(compare("1.2.0", "1.3.0"), VersionRelation::UpdatePlugin);
        assert_eq!(compare("2.0.0", "1.9.0"), VersionRelation::Incompatible);
        assert_eq!(compare("v1.2.3", "1.2.3"), VersionRelation::Unknown);
    }
}
