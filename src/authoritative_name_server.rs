use std::{net::Ipv4Addr, process::Child};

use crate::{container::Container, Domain, Result, CHMOD_RW_EVERYONE};

pub struct AuthoritativeNameServer {
    child: Child,
    container: Container,
}

impl AuthoritativeNameServer {
    pub fn start(domain: Domain) -> Result<Self> {
        let container = Container::run()?;

        container.status_ok(&["mkdir", "-p", "/etc/nsd/zones"])?;

        let zone_path = "/etc/nsd/zones/main.zone";
        container.cp(
            "/etc/nsd/nsd.conf",
            &nsd_conf(domain.fqdn()),
            CHMOD_RW_EVERYONE,
        )?;

        let zone_file_contents = match domain {
            Domain::Root => root_zone(),
            Domain::Tld { domain } => tld_zone(domain),
        };

        container.cp(zone_path, &zone_file_contents, CHMOD_RW_EVERYONE)?;

        let child = container.spawn(&["nsd", "-d"])?;

        Ok(Self { child, container })
    }

    pub fn ipv4_addr(&self) -> Ipv4Addr {
        self.container.ipv4_addr()
    }
}

impl Drop for AuthoritativeNameServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn tld_zone(domain: &str) -> String {
    assert!(domain.ends_with('.'));
    assert!(!domain.starts_with('.'));

    minijinja::render!(
        include_str!("templates/tld.zone.jinja"),
        tld => domain,
    )
}

fn root_zone() -> String {
    minijinja::render!(include_str!("templates/root.zone.jinja"),)
}

fn nsd_conf(domain: &str) -> String {
    assert!(domain.ends_with('.'));

    minijinja::render!(
        include_str!("templates/nsd.conf.jinja"),
        domain => domain
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tld_setup() -> Result<()> {
        let tld_ns = AuthoritativeNameServer::start(Domain::Tld { domain: "com." })?;
        let ip_addr = tld_ns.ipv4_addr();

        let client = Container::run()?;
        let output = client.output(&["dig", &format!("@{ip_addr}"), "SOA", "com."])?;

        assert!(output.status.success());
        eprintln!("{}", output.stdout);
        assert!(output.stdout.contains("status: NOERROR"));

        Ok(())
    }

    #[test]
    fn root_setup() -> Result<()> {
        let root_ns = AuthoritativeNameServer::start(Domain::Root)?;
        let ip_addr = root_ns.ipv4_addr();

        let client = Container::run()?;
        let output = client.output(&["dig", &format!("@{ip_addr}"), "SOA", "."])?;

        assert!(output.status.success());
        eprintln!("{}", output.stdout);
        assert!(output.stdout.contains("status: NOERROR"));

        Ok(())
    }
}
