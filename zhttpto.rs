//
// zhttpto.rs
//
// Starting code for PS1
// Running on Rust 0.9
//
// Note that this code has serious security risks!  You should not run it 
// on any system with access to sensitive files.
// 
// University of Virginia - cs4414 Spring 2014
// Weilin Xu and David Evans
// Version 0.3

//Author: Hangchen Qu
//id: hq5na
//date: Jan.26 2014

#[feature(globs)];
use std::io::*;
use std::io::net::ip::{SocketAddr};
use std::{str};


static IP: &'static str = "127.0.0.1";
static PORT:        int = 4414;
static mut visitor_count: uint = 0;

fn main() {
    let addr = from_str::<SocketAddr>(format!("{:s}:{:d}", IP, PORT)).unwrap();
    let mut acceptor = net::tcp::TcpListener::bind(addr).listen();
    
    println(format!("Listening on [{:s}] ...", addr.to_str()));
    
    for stream in acceptor.incoming() {
        // Spawn a task to handle the connection
        do spawn {
            let mut stream = stream;
            
            match stream {
                Some(ref mut s) => {
                             match s.peer_name() {
                                Some(pn) => {println(format!("Received connection from: [{:s}]", pn.to_str()));},
                                None => ()
                             }
                           },
                None => ()
            }        
            unsafe{
            	visitor_count += 1;
            }
            let mut buf = [0, ..500];
            stream.read(buf);
            let request_str = str::from_utf8(buf);
            println(format!("Received request :\n{:s}", request_str));
            let request_line: ~[&str] = request_str.split(' ').collect();
            
            if request_line.len() > 2{
            	let mut path_name: &str = request_line[1];
            	println!("Request for path: {:s}", path_name);
            
            	let mut file_path = std::os::getcwd();
            	let mut w: &str = path_name;
            	let mut w2 = path_name.slice_from(1).to_owned();
            	file_path.push(w2);
            	
            	println!("Request file path: {}", file_path.display());
	
	            if !file_path.exists() || file_path.is_dir(){
	            	unsafe{
	            	let response: ~str = 
	                ~"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n
	                 <doctype !html><html><head><title>Hello, Rust!</title>
	                 <style>body { background-color: #111; color: #FFEEAA }
	                        h1 { font-size:2cm; text-align: center; color: black; text-shadow: 0 0 4mm red}
	                        h2 { font-size:2cm; text-align: center; color: black; text-shadow: 0 0 4mm green}
	                 </style></head>
	                 <body>
	                 <h1>Greetings, Krusty!</h1>"
	                 + ~"<h2>Visitor count: " + visitor_count.to_str()
	                 + ~"</h2> No file served.</body></html>\r\n";
	            	stream.write(response.as_bytes());
	           	 	println!("Connection terminates.");
	           	 	println!("No file served");
	            	} 	
	            }else{
	            	println!("serve file: {}", file_path.display());
	            	
	            	let serve_file = File::open(&file_path);
	            	let mut xx = file_path.extension_str();
	        
	            	println!("extension is: {}", xx)
	            	match (serve_file) {
            			Some(mut serve) => {
            				
            				if xx == Some("html"){	
	               					let serve_bytes: ~[u8] = serve.read_to_end();
	        						stream.write(serve_bytes);
	        					}
	        				else{
				let response2: ~str = 
                ~"HTTP/1.1 403 Forbidden\r\nContent-Type: text/html\r\n\r\n<!DOCTYPE html>\r\n<html>\r\n\t<head>\r\n\t\t<title>403 - Forbidden</title>\r\n\t</head>\r\n\t<body>\r\n\t\t<h1>403 - Forbidden</h1>\r\n\t</body>\r\n</html>";
            stream.write(response2.as_bytes());
            	println!("Forbidden");
	        				}
            			} ,
            			None => fail!("Error opening message file")
        			}
	            	
	            }
	            
	            
	        }
        }
    }
}