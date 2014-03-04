//
// zhtta.rs
//
// Starting code for PS3
// Running on Rust 0.9
//
// Note that this code has serious security risks!  You should not run it 
// on any system with access to sensitive files.
// 
// University of Virginia - cs4414 Spring 2014
// Weilin Xu and David Evans
// Version 0.5

// To see debug! outputs set the RUST_LOG environment variable, e.g.: export RUST_LOG="zhtta=debug"

#[feature(globs)];
extern mod extra;

use std::io::*;
use std::io::net::ip::{SocketAddr};
use std::{os, str, libc, from_str};
use std::path::Path;
use std::hashmap::HashMap;
use std::num::ToPrimitive;

use extra::getopts;
use extra::arc::MutexArc;
use extra::arc::RWArc;
use gash::*;
use extra::lru_cache::LruCache;

mod gash;

static SERVER_NAME : &'static str = "Zhtta Version 0.5";

static IP : &'static str = "127.0.0.1";
static PORT : uint = 4414;
static WWW_DIR : &'static str = "./www";

static HTTP_OK : &'static str = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n";
static HTTP_BAD : &'static str = "HTTP/1.1 404 Not Found\r\n\r\n";

static COUNTER_STYLE : &'static str = "<doctype !html><html><head><title>Hello, Rust!</title>
             <style>body { background-color: #884414; color: #FFEEAA}
                    h1 { font-size:2cm; text-align: center; color: black; text-shadow: 0 0 4mm red }
                    h2 { font-size:2cm; text-align: center; color: black; text-shadow: 0 0 4mm green }
             </style></head>
             <body>";

//static mut visitor_count : uint = 0;


struct HTTP_Request {
    // Use peer_name as the key to access TcpStream in hashmap. 

    // (Due to a bug in extra::arc in Rust 0.9, it is very inconvenient to use TcpStream without the "Freeze" bound.
    //  See issue: https://github.com/mozilla/rust/issues/12139)
    peer_name: ~str,
    path: ~Path,
}

struct WebServer {
    ip: ~str,
    port: uint,
    www_dir_path: ~Path,
    
    request_queue_arc: MutexArc<~[HTTP_Request]>,
    stream_map_arc: MutexArc<HashMap<~str, Option<std::io::net::tcp::TcpStream>>>,
    
    notify_port: Port<()>,
    shared_notify_chan: SharedChan<()>,
//added
    visitor_count : RWArc<int>,
    task_count : RWArc<int>,
 	cache_map_arc: MutexArc<LruCache<~str, ~[u8]>>,
 	cache: LruCache<~str, ~[u8]>,
}

impl WebServer {
    fn new(ip: &str, port: uint, www_dir: &str) -> WebServer {
        let (notify_port, shared_notify_chan) = SharedChan::new();
        let www_dir_path = ~Path::new(www_dir);
        os::change_dir(www_dir_path.clone());

        WebServer {
            ip: ip.to_owned(),
            port: port,
            www_dir_path: www_dir_path,
                        
            request_queue_arc: MutexArc::new(~[]),
            stream_map_arc: MutexArc::new(HashMap::new()),
            
            notify_port: notify_port,
            shared_notify_chan: shared_notify_chan,      
//Added 
            visitor_count : RWArc::new(0),
            task_count: RWArc::new(16),
            cache_map_arc: MutexArc::new(LruCache::new(4)),
 		    cache: LruCache::new(4),
        }
    }
    
    fn run(&mut self) {
        self.listen();
        self.dequeue_static_file_request();
    }
    
    fn listen(&mut self) {
        let addr = from_str::<SocketAddr>(format!("{:s}:{:u}", self.ip, self.port)).expect("Address error.");
        let www_dir_path_str = self.www_dir_path.as_str().expect("invalid www path?").to_owned();
        
        let request_queue_arc = self.request_queue_arc.clone();
        let shared_notify_chan = self.shared_notify_chan.clone();
        let stream_map_arc = self.stream_map_arc.clone();
        
//added
		let visitor_count_clone = self.visitor_count.clone();
                
        spawn(proc() {
            let mut acceptor = net::tcp::TcpListener::bind(addr).listen();
            println!("{:s} listening on {:s} (serving from: {:s}).", 
                     SERVER_NAME, addr.to_str(), www_dir_path_str);
			
            for stream in acceptor.incoming() {
                let (queue_port, queue_chan) = Chan::new();
                queue_chan.send(request_queue_arc.clone());
                
                let notify_chan = shared_notify_chan.clone();
                let stream_map_arc = stream_map_arc.clone();
                
//added
				let visitor_count_clone = visitor_count_clone.clone();
                // Spawn a task to handle the connection.
                spawn(proc() {
//                  unsafe { visitor_count += 1; } // TODO: Fix unsafe counter
                    visitor_count_clone.write(|current| {*current +=1});
                    let request_queue_arc = queue_port.recv();
                  
                    let mut stream = stream;
                    
                    let peer_name = WebServer::get_peer_name(&mut stream);
                    
                    let mut buf = [0, ..500];
                    stream.read(buf);
                    let request_str = str::from_utf8(buf);
                    debug!("Request:\n{:s}", request_str);
                    
                    let req_group : ~[&str]= request_str.splitn(' ', 3).collect();
                    if req_group.len() > 2 {
                        let path_str = "." + req_group[1].to_owned();
                        
                        let mut path_obj = ~os::getcwd();
                        path_obj.push(path_str.clone());
                        
                        let ext_str = match path_obj.extension_str() {
                            Some(e) => e,
                            None => "",
                        };
                        
                        debug!("Requested path: [{:s}]", path_obj.as_str().expect("error"));
                        debug!("Requested path: [{:s}]", path_str);
                             
                        if path_str == ~"./" {
                            debug!("===== Counter Page request =====");
//edited                            
                            WebServer::respond_with_counter_page(visitor_count_clone.clone(),stream);	//added
                            debug!("=====Terminated connection from [{:s}].=====", peer_name);
                        } else if !path_obj.exists() || path_obj.is_dir() {
                            debug!("===== Error page request =====");
                            WebServer::respond_with_error_page(stream, path_obj);
                            debug!("=====Terminated connection from [{:s}].=====", peer_name);
                        } else if ext_str == "shtml" { // Dynamic web pages.
                            debug!("===== Dynamic Page request =====");
                            WebServer::respond_with_dynamic_page(stream, path_obj);
                            debug!("=====Terminated connection from [{:s}].=====", peer_name);
                        } else { 
                            debug!("===== Static Page request =====");
                            WebServer::enqueue_static_file_request(stream, path_obj, stream_map_arc, request_queue_arc, notify_chan);
                        }
                    }
                });
            }
        });
    }

    fn respond_with_error_page(stream: Option<std::io::net::tcp::TcpStream>, path: &Path) {
        let mut stream = stream;
        let msg: ~str = format!("Cannot open: {:s}", path.as_str().expect("invalid path").to_owned());

        stream.write(HTTP_BAD.as_bytes());
        stream.write(msg.as_bytes());
    }

    // TODO: Safe visitor counter.
    fn respond_with_counter_page(visitor_count: RWArc<int>, stream: Option<std::io::net::tcp::TcpStream>) {
        let mut stream = stream;
        let mut temp = 0;
        visitor_count.read(|count| {temp = *count;});
        
        let response: ~str = 
            format!("{:s}{:s}<h1>Greetings, Krusty!</h1>
                     <h2>Visitor count: {:?}</h2></body></html>\r\n", 
                    HTTP_OK, COUNTER_STYLE, 
//                    unsafe { visitor_count } );
					temp);
        debug!("Responding to counter request");
        stream.write(response.as_bytes());
    }
    
    // Done: Streaming file.
//added get file size
    fn get_size(path: &Path) -> uint{
    	let fsize = path.stat();
    	fsize.size.to_uint().unwrap()
    }
    
    // Done: Application-layer file caching.

    fn respond_with_static_file(stream: Option<std::io::net::tcp::TcpStream>, path: &Path, cache: MutexArc<LruCache<~str, ~[u8]>>) {
// added steaming method & caching
        let mut stream = stream;
        //let mut file_reader = File::open(path).expect("Invalid file!").to_owned();
        stream.write(HTTP_OK.as_bytes());
		//let file_size: uint = WebServer::get_size(path);	
		let path_name: ~str = path.as_str().expect("Invalid file.").to_owned();

 		let mut already_read: uint = 0;
 		let block_size: uint = 10240;
 		       
		cache.access( |cache_map| {
 			let mut buffer: ~[u8] = ~[];
        	match cache_map.get(&path_name) {
        	 	None => {
 					let mut file_reader = File::open(path).expect("Invalid file!");
 					let file_size: uint = WebServer::get_size(path);
 					
 					while block_size < (file_size - already_read) {
 						let block = file_reader.read_bytes(block_size);
 						stream.write(block);
 						already_read += block_size;
 						buffer.push_all_move(block);
 					}//while
 					let block = file_reader.read_to_end();
         			stream.write(block);
 					buffer.push_all_move(block);
 				}//None
 				
 				Some(buff) => {
 					let file_size: uint = buff.len();
 			
 					while block_size < (file_size - already_read) {
 						let block = buff.slice(already_read, already_read + block_size);
 						stream.write(block);
 						already_read += block_size;
 					}//while
    				stream.write(buff.slice(already_read, file_size));
				}//Some
		 	}//match
	 		if buffer.len() > 0 {
 				cache_map.put(path_name.clone(), buffer);
 			}//if
 			
    	}); //cache
               
 	}//fn respond with static file
 	
//added
	fn respond_with_dynamic_file(stream: Option<std::io::net::tcp::TcpStream>, path: &Path) {
 		let mut stream = stream;
        let mut file_reader = File::open(path).expect("Invalid file!");
		let file_bytes = file_reader.read_to_end();
        stream.write(HTTP_OK.as_bytes());

//dynamic_file

 		let file_string = str::from_utf8(file_bytes);
 		let mut stream_output = file_string.to_owned(); 		
 		for line in file_string.lines() { //read
 			if line.contains("<!--#") {
 				let read_line = line.slice(line.find_str("<!--#").unwrap(), line.find_str("-->").unwrap()+3);
 				if (read_line.contains("#exec cmd=")) {
 					let command = read_line.slice(read_line.find('"').unwrap()+1, read_line.rfind('"').unwrap()); //get cmd from input
 					let output = gash::run_cmdline(command);
 					stream_output = stream_output.replace(read_line, output); //replace output
 				}//if
 			}//if
 		}//for
         stream.write_str(stream_output);
    }
    
    // Done: Server-side gashing.
    fn respond_with_dynamic_page(stream: Option<std::io::net::tcp::TcpStream>, path: &Path) {
          // for now, just serve as static file
 //        WebServer::respond_with_static_file(stream, path);
       WebServer::respond_with_dynamic_file(stream, path); //changed to dynamic
    }
    
    // TODO: Smarter Scheduling.
    fn enqueue_static_file_request(stream: Option<std::io::net::tcp::TcpStream>, path_obj: &Path, stream_map_arc: MutexArc<HashMap<~str, Option<std::io::net::tcp::TcpStream>>>, req_queue_arc: MutexArc<~[HTTP_Request]>, notify_chan: SharedChan<()>) {
        // Save stream in hashmap for later response.
        let mut stream = stream;
        let peer_name = WebServer::get_peer_name(&mut stream);
        let (stream_port, stream_chan) = Chan::new();
        stream_chan.send(stream);
        unsafe {
            // Use an unsafe method, because TcpStream in Rust 0.9 doesn't have "Freeze" bound.
            stream_map_arc.unsafe_access(|local_stream_map| {
                let stream = stream_port.recv();
                local_stream_map.swap(peer_name.clone(), stream);
            });
        }
        
        // Enqueue the HTTP request.
        let req = HTTP_Request { peer_name: peer_name.clone(), path: ~path_obj.clone() };
        let (req_port, req_chan) = Chan::new();
        req_chan.send(req);

        debug!("Waiting for queue mutex lock.");
        req_queue_arc.access(|local_req_queue| {
            debug!("Got queue mutex lock.");
            let req: HTTP_Request = req_port.recv();
//            local_req_queue.push(req);
//added
//check ip address, split by dot and then check first two values of ip. if satisfy then prioritize
//s			println!("{:?}", WebServer::get_size(path_obj)); 			
 			let peer_value = peer_name.split('.').to_owned_vec();
 			let ip_vec = peer_value[0] + "." + peer_value[1];
 			
 			if (str::eq(&ip_vec, &~"128.143")) {
 				local_req_queue.unshift(req);
 			}
 			else if (str::eq(&ip_vec, &~"137.54")){
 				local_req_queue.unshift(req); 				
 			}
 			else if (str::eq(&ip_vec, &~"71.206")){
 				local_req_queue.unshift(req); 				
 			}
 			else if (str::eq(&ip_vec, &~"127.143")){
 				local_req_queue.unshift(req);
 			}
			else {
 				local_req_queue.push(req);
	 		}		 		
            debug!("A new request enqueued, now the length of queue is {:u}.", local_req_queue.len());
        });
        
        notify_chan.send(()); // Send incoming notification to responder task.
    
    
    }
    
    // TODO: Smarter Scheduling.
    fn dequeue_static_file_request(&mut self) {
        let req_queue_get = self.request_queue_arc.clone();
        let stream_map_get = self.stream_map_arc.clone();
//added
		let get_task_count = self.task_count.clone();
		let cache_map_get = self.cache_map_arc.clone();
        // Port<> cannot be sent to another task. So we have to make this task as the main task that can access self.notify_port.
        
        let (request_port, request_chan) = Chan::new();
        loop {
            self.notify_port.recv();    // waiting for new request enqueued.
            
            req_queue_get.access( |req_queue| {
                match req_queue.shift_opt() { // FIFO queue.
                    None => { /* do nothing */ }
                    Some(req) => {
                        request_chan.send(req);
                        debug!("A new request dequeued, now the length of queue is {:u}.", req_queue.len());
                    }
                }
            });
            
            let request = request_port.recv();
            
            // Get stream from hashmap.
            // Use unsafe method, because TcpStream in Rust 0.9 doesn't have "Freeze" bound.
            let (stream_port, stream_chan) = Chan::new();
            unsafe {
                stream_map_get.unsafe_access(|local_stream_map| {
                    let stream = local_stream_map.pop(&request.peer_name).expect("no option tcpstream");
                    stream_chan.send(stream);
                });
            }
            
            // TODO: Spawning more tasks to respond the dequeued requests concurrently. You may need a semophore to control the concurrency.
			//original code
            //let stream = stream_port.recv();
            //WebServer::respond_with_static_file(stream, request.path);
            //debug!("=====Terminated connection from [{:s}].=====", request.peer_name);

//added tasks            
            let mut temp_task = 0;
            get_task_count.read(|count| {temp_task = *count;});
			if (temp_task > 0){
				let task_count_clone = get_task_count.clone();
				let cache_map_clone = cache_map_get.clone();
				spawn(proc(){
					//println!("{:?}", *count);}
					task_count_clone.write(|count| {*count -= 1;});

					let stream = stream_port.recv();	
		 			WebServer::respond_with_static_file(stream, request.path, cache_map_clone);

		 			// Close stream automatically.
	 		        debug!("=====Terminated connection from [{:s}].=====", request.peer_name);
		 			task_count_clone.write(|count| {*count += 1;});
				});
			}           
        }
    }
    
    fn get_peer_name(stream: &mut Option<std::io::net::tcp::TcpStream>) -> ~str {
        match *stream {
            Some(ref mut s) => {
                         match s.peer_name() {
                            Some(pn) => {pn.to_str()},
                            None => (~"")
                         }
                       },
            None => (~"")
        }
    }
}

fn get_args() -> (~str, uint, ~str) {
    fn print_usage(program: &str) {
        println!("Usage: {:s} [options]", program);
        println!("--ip     \tIP address, \"{:s}\" by default.", IP);
        println!("--port   \tport number, \"{:u}\" by default.", PORT);
        println!("--www    \tworking directory, \"{:s}\" by default", WWW_DIR);
        println("-h --help \tUsage");
    }
    
    /* Begin proceread_lineng program arguments and initiate the parameters. */
    let args = os::args();
    let program = args[0].clone();
    
    let opts = ~[
        getopts::optopt("ip"),
        getopts::optopt("port"),
        getopts::optopt("www"),
        getopts::optflag("h"),
        getopts::optflag("help")
    ];

    let matches = match getopts::getopts(args.tail(), opts) {
        Ok(m) => { m }
        Err(f) => { fail!(f.to_err_msg()) }
    };

    if matches.opt_present("h") || matches.opt_present("help") {
        print_usage(program);
        unsafe { libc::exit(1); }
    }
    
    let ip_str = if matches.opt_present("ip") {
                    matches.opt_str("ip").expect("invalid ip address?").to_owned()
                 } else {
                    IP.to_owned()
                 };
    
    let port:uint = if matches.opt_present("port") {
                        from_str::from_str(matches.opt_str("port").expect("invalid port number?")).expect("not uint?")
                    } else {
                        PORT
                    };
    
    let www_dir_str = if matches.opt_present("www") {
                        matches.opt_str("www").expect("invalid www argument?") 
                      } else { WWW_DIR.to_owned() };
    
    (ip_str, port, www_dir_str)
}

fn main() {
    let (ip_str, port, www_dir_str) = get_args();
    let mut zhtta = WebServer::new(ip_str, port, www_dir_str);
    zhtta.run();
}
