//! Switch Discovery & Probing Tool
//!
//! Finds Cisco switch on local network and probes capabilities

use std::net::{TcpStream, UdpSocket, IpAddr, Ipv4Addr};
use std::time::{Duration, Instant};
use std::io::{Read, Write};

fn main() {
    println!("=== FBC Switch Probe ===\n");

    // Step 1: Find the switch
    println!("[1] Scanning 172.16.0.0/24 for switch...");
    if let Some(switch_ip) = find_switch() {
        println!("    Found switch at: {}\n", switch_ip);

        // Step 2: Try to connect
        println!("[2] Attempting Telnet (23) and SSH (22)...");
        probe_management(switch_ip);

        // Step 3: ARP scan to see MAC table
        println!("\n[3] Sending ARP to populate switch CAM...");
        arp_scan(switch_ip);

        // Step 4: Broadcast test
        println!("\n[4] Testing broadcast forwarding...");
        test_broadcast();

    } else {
        println!("    No switch found. Trying common default IPs...");
        for ip in &["192.168.1.1", "10.0.0.1", "172.16.0.1"] {
            println!("    Trying {}...", ip);
            if let Ok(ip_addr) = ip.parse::<Ipv4Addr>() {
                probe_management(ip_addr);
            }
        }
    }
}

/// Scan network for devices responding on common switch ports
fn find_switch() -> Option<Ipv4Addr> {
    // Common switch management ports
    let ports = [23, 22, 80, 443]; // Telnet, SSH, HTTP, HTTPS

    // Scan 172.16.0.1 - 172.16.0.254
    for i in 1..255 {
        let ip = Ipv4Addr::new(172, 16, 0, i);

        // Skip our own IP
        if i == 49 {
            continue;
        }

        // Try each port with short timeout
        for &port in &ports {
            if let Ok(_) = TcpStream::connect_timeout(
                &format!("{}:{}", ip, port).parse().unwrap(),
                Duration::from_millis(100)
            ) {
                println!("    -> Found device at {}:{}", ip, port);
                return Some(ip);
            }
        }

        // Progress indicator
        if i % 50 == 0 {
            print!(".");
            std::io::stdout().flush().unwrap();
        }
    }

    None
}

/// Try to connect to switch management interface
fn probe_management(ip: Ipv4Addr) {
    // Try Telnet (port 23)
    print!("    Telnet (23): ");
    match TcpStream::connect_timeout(
        &format!("{}:23", ip).parse().unwrap(),
        Duration::from_secs(2)
    ) {
        Ok(mut stream) => {
            println!("OPEN");

            // Try to read banner
            let mut buf = [0u8; 1024];
            stream.set_read_timeout(Some(Duration::from_secs(1))).ok();
            if let Ok(n) = stream.read(&mut buf) {
                let banner = String::from_utf8_lossy(&buf[..n]);
                println!("    Banner: {}", banner.trim());

                // Try default Cisco credentials
                println!("    Trying default credentials...");

                // Common defaults:
                // - cisco/cisco
                // - admin/admin
                // - (blank)/(blank)

                // Wait for username prompt
                std::thread::sleep(Duration::from_millis(500));

                // Try blank login first (just press enter)
                stream.write_all(b"\n").ok();
                std::thread::sleep(Duration::from_millis(500));

                let mut buf = [0u8; 2048];
                if let Ok(n) = stream.read(&mut buf) {
                    let response = String::from_utf8_lossy(&buf[..n]);
                    println!("    Response:\n{}", response);

                    if response.contains("Password:") {
                        println!("    Needs password. Try 'cisco' or 'admin'");
                    } else if response.contains(">") || response.contains("#") {
                        println!("    Got shell prompt! (no password)");

                        // Try show version
                        stream.write_all(b"show version\n").ok();
                        std::thread::sleep(Duration::from_millis(500));

                        let mut buf = [0u8; 4096];
                        if let Ok(n) = stream.read(&mut buf) {
                            let output = String::from_utf8_lossy(&buf[..n]);
                            println!("\n=== Show Version ===\n{}", output);
                        }
                    }
                }
            }
        }
        Err(_) => println!("CLOSED"),
    }

    // Try SSH (port 22)
    print!("    SSH (22): ");
    match TcpStream::connect_timeout(
        &format!("{}:22", ip).parse().unwrap(),
        Duration::from_secs(2)
    ) {
        Ok(_) => println!("OPEN (use 'ssh {}' to connect)", ip),
        Err(_) => println!("CLOSED"),
    }

    // Try HTTP (port 80)
    print!("    HTTP (80): ");
    match TcpStream::connect_timeout(
        &format!("{}:80", ip).parse().unwrap(),
        Duration::from_secs(2)
    ) {
        Ok(_) => println!("OPEN (try http://{} in browser)", ip),
        Err(_) => println!("CLOSED"),
    }
}

/// Send ARP to populate switch CAM table
fn arp_scan(switch_ip: Ipv4Addr) {
    println!("    Sending ARP requests to entire subnet...");

    // Note: Sending raw ARP requires elevated privileges
    // For now, just try ICMP ping which will trigger ARP

    use std::process::Command;

    for i in 1..255 {
        if i == 49 { continue; } // Skip our IP

        let ip = format!("172.16.0.{}", i);

        // Fire and forget ping (don't wait for response)
        #[cfg(windows)]
        Command::new("ping")
            .args(&["-n", "1", "-w", "10", &ip])
            .stdout(std::process::Stdio::null())
            .spawn()
            .ok();

        #[cfg(unix)]
        Command::new("ping")
            .args(&["-c", "1", "-W", "1", &ip])
            .stdout(std::process::Stdio::null())
            .spawn()
            .ok();
    }

    println!("    Sent ARP/ping to all 254 addresses");
    println!("    Switch CAM table should now be populated");
}

/// Test broadcast forwarding performance
fn test_broadcast() {
    println!("    Creating UDP socket for broadcast test...");

    match UdpSocket::bind("172.16.0.49:9999") {
        Ok(socket) => {
            socket.set_broadcast(true).unwrap();
            socket.set_read_timeout(Some(Duration::from_secs(1))).unwrap();

            println!("    Sending broadcast packet...");
            let payload = b"FBC_BROADCAST_TEST";
            let start = Instant::now();

            // Send to broadcast address
            socket.send_to(payload, "172.16.0.255:9999").unwrap();

            println!("    Sent in {:?}", start.elapsed());
            println!("    Waiting for echoes...");

            // Listen for any responses
            let mut buf = [0u8; 1024];
            let mut count = 0;

            loop {
                match socket.recv_from(&mut buf) {
                    Ok((n, src)) => {
                        println!("    <- Received {} bytes from {}", n, src);
                        count += 1;
                    }
                    Err(_) => break, // Timeout
                }
            }

            println!("    Received {} responses", count);
        }
        Err(e) => {
            println!("    Failed to create socket: {}", e);
            println!("    (May need admin/root privileges)");
        }
    }
}
