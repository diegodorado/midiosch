use nannou_osc as osc;
use nannou_osc::Type;
use nannou_osc::Sender;
use nannou_osc::Connected;
use midir::{MidiInput, MidiInputConnection, Ignore};
use clap;
use std::collections::HashMap;

use tui::{
    layout::{Constraint, Direction, Layout, Alignment},
    style::{Style},
    widgets::{
        Block, Borders, Paragraph, Row, Table,
    },
    text::{Text,Span},
    Terminal,
    backend::CrosstermBackend,
};

use crossterm::{
    event::{self, EnableMouseCapture, Event as CEvent, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};

use std::{
    error::Error,
    io::{stdin, stdout, Write},
    thread,
    time::{Duration, Instant},
};
use crossbeam_channel::unbounded;


enum Event<I> {
    Input(I),
    Tick,
    MidiEvent(Vec<u8>),
}

#[derive(Debug)]
enum MidiEvent {
    ControlChange(u8,u8,u8),
    NoteOn(u8,u8,u8),
    NoteOff(u8,u8),
}

struct App {
    midi_input_port_name: String,
    midi_connection: Option<MidiInputConnection<()>>,
    osc_sender: Sender<Connected>,
    normalize: bool,
    events: HashMap<String,(Instant,String)>,
}

impl App {
    fn setup(midi_input_port_name: String, osc_sender: Sender<Connected>, normalize : bool) -> App {
        App {
            midi_input_port_name,
            midi_connection: None,
            osc_sender,
            normalize,
            events: HashMap::new(),
        }
    }

    fn update(&mut self) {

        let keep_for = Duration::from_millis(10000);

        let oldies: Vec<_> = self.events
            .iter()
            .filter(|(_,(t,_))| t.elapsed() > keep_for)
            .map(|(k, _)| k.clone())
            .collect();

        for old in oldies { self.events.remove(&old);}

    }

    fn on_midi(&mut self, ev: MidiEvent) {

        let addr;
        let ivalue;
        let mut args = Vec::new();

        match ev {
            MidiEvent::NoteOff(ch,note) => {
                addr = format!("/note/{}/{}", ch, note);
                ivalue = 0;
            }
            MidiEvent::NoteOn(ch,note,vel) => {
                addr = format!("/note/{}/{}", ch, note);
                ivalue = vel.into();
            }
            MidiEvent::ControlChange(ch,num,val) => {
                addr = format!("/cc/{}/{}", ch, num);
                ivalue = val.into();
            }
        };

        let now = Instant::now();
        if self.normalize {
            let value = (ivalue as f32) / 127.0;
            args.push(Type::Float(value));
            self.events.insert(addr.clone(), (now,value.to_string()));
        }else{
            args.push(Type::Int(ivalue));
            self.events.insert(addr.clone(), (now,ivalue.to_string()));
        }
        let packet = (addr, args);
        self.osc_sender.send(packet).ok();
    }

}


fn main() -> Result<(), Box<dyn Error>> {

    let logo = r#"
                            :                         
                          `sMh`                       
                         `hMNMd.                      
                        .dMd.yMN:                     
                       :mMy`  oNN+                    
                      +NNo     /NMs`                  
                    `sMN/       -mMh.                 
                   `hMm-         .hMd-                
                  -dMh.           `yMm:               
                 :mMy`   `.....`    oNN+              
                +NNo`-+shdddhhhddy+-`/NMs`            
              `sMNssdmdyo+//////+shmmyomMh.           
             .hMMNNMmdhmhomMMMNssmhhmMMNMMd-          
            -dMMMmho:..N+`NMMMM+`Mo.-+ymMMMm:         
           :mMhmMNo.   sm-/yhho-ym.  .+mMmyNN+        
          +NM+``/hNNh+-`/yhsosyho.-+hNNd+. /NMs`      
        `sMN/     .+hmNmdyssyssyhmNNho-`    -mMh`     
       `hMm-         `-/syhdddhys+-`         .hMd-    
      .mMh.                                   `sMN:   
     :NMMyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyNMN+  
    `ossssssssssssssssssssssssssssssssssssssssssssso- 
    "#;

    let matches = clap::App::new("MIDIOSCH")
        .version("0.1")
        .author("Diego Dorado <diegodorado@gmail.com>")
        .about("Forwards MIDI note and cc to OSC.")
        .arg(clap::Arg::new("port")
            .short('p')
            .long("port")
            .value_name("PORT")
            .about("Sets the OSC port to use.")
            .takes_value(true))
        .arg(clap::Arg::new("input")
            .short('i')
            .long("input")
            .value_name("MIDI_INPUT_INDEX")
            .about("Sets the MIDI device index to use.")
            .takes_value(true))
        .arg(clap::Arg::new("normalize")
            .short('n')
            .long("normalize")
            .value_name("NORMALIZE")
            .about("Whether to normalize 0-127 values to a float between 0 and 1.")
            .takes_value(true))
        .get_matches();

    let port: u16 = matches.value_of_t("port").unwrap_or(9000);
    let midi_input: Option<usize> = match matches.value_of_t("input") {
        Ok(v) => Some(v),
        Err(_) => None,
    };
    let normalize: bool = matches.value_of_t("normalize").unwrap_or(true);

    let target_addr = format!("{}:{}", "127.0.0.1", port);
    let osc_sender = osc::sender()
        .expect("Could not bind to default socket")
        .connect(target_addr)
        .expect("Could not connect to socket at address");

    let mut midi_in = MidiInput::new("midir reading input")?;
    midi_in.ignore(Ignore::None);
    // Get an input port (read from console if multiple are available)
    let in_ports = midi_in.ports();
    let in_port = match in_ports.len() {
        0 => return Err("no input port found".into()),
        _ => {
            match midi_input {
                Some(index) => {
                    in_ports.get(index).ok_or("invalid input index ")?
                }
                None => {
                    if in_ports.len() == 1 {
                        &in_ports[0]
                    }
                    else{
                        println!("\nAvailable input ports:");
                        for (i, p) in in_ports.iter().enumerate() {
                            println!("{}: {}", i, midi_in.port_name(p).unwrap());
                        }
                        print!("Please select input port: ");
                        stdout().flush()?;
                        let mut input = String::new();
                        stdin().read_line(&mut input)?;
                        in_ports.get(input.trim().parse::<usize>()?)
                                 .ok_or("invalid input port selected")?
                    }
                }
            }
        }
    };


    let portname =  format!("{}", midi_in.port_name(in_port).unwrap());

    let mut app = App::setup(portname, osc_sender, normalize);

    // Setup input handling
    let (tx, rx) = unbounded();

    {
        let tx = tx.clone();
        let tick_rate = Duration::from_millis(60);
        thread::spawn(move || {
            let mut last_tick = Instant::now();
            loop {
                // poll for tick rate duration, if no events, sent tick event.
                let timeout = tick_rate
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or_else(|| Duration::from_secs(0));
                if event::poll(timeout).unwrap() {
                    if let CEvent::Key(key) = event::read().unwrap() {
                        tx.send(Event::Input(key)).unwrap();
                    }
                }
                if last_tick.elapsed() >= tick_rate {
                    tx.send(Event::Tick).unwrap();
                    last_tick = Instant::now();
                }
            }
        });
    }


    // _conn_in needs to be a named parameter, because it needs to be kept alive until the end of the scope
    app.midi_connection = match midi_in.connect(
        in_port,
        "midir-read-input",
        move |_, msg, _| {
            tx.send(Event::MidiEvent(msg.to_vec())).unwrap();
        },
        (),
    ){
        Ok(v) => Some(v),
        Err(_) => None,
    };

    enable_raw_mode()?;
    // Terminal initialization
    let mut _stdout = stdout();
    execute!(_stdout, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(_stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.clear()?;

    let left_text = Text::from(format!(
            r#"{}
            
            Connected to {}

            Sending OSC to 127.0.0.1:{}

            Press 'Q' to exit. 
            "#, logo, app.midi_input_port_name, port));

    'main: loop {

        //fixme: cant find a way to consume all messages
        //without hoging the cpu
        //... tried with try_recv(),
        //  and messages are not delayed, but it is to heavy on the cpu
        match rx.recv()? {
            Event::Input(event) => match event.code {
                KeyCode::Char('q') => {
                    break 'main;
                }
                _ => {}
            },
            Event::Tick => {
                app.update();

                terminal.draw(|f| {

                    let create_block = |title| {
                        Block::default()
                            .borders(Borders::ALL)
                            .title(Span::styled(title, Style::default()))
                    };
                    let paragraph = Paragraph::new(left_text.clone())
                        .block(create_block("MIDIOSCH"))
                        .alignment(Alignment::Center);
                    let chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                        .split(f.size());
                    f.render_widget(paragraph, chunks[0]);
                    let style = Style::default();

                    let mut rows = Vec::new();
                    for (k,(_,v)) in app.events.iter() {
                        let row = vec![k,v];
                        rows.push(Row::StyledData(row.into_iter(), style) );
                    }

                    let table = Table::new(["ADDR", "VAL"].iter(), rows.into_iter())
                    .block(Block::default().borders(Borders::ALL).title("EVENTS"))
                    .widths(&[
                        Constraint::Min(13),
                        Constraint::Min(5),
                    ]);

                    f.render_widget(table, chunks[1]);
                });

            },
            Event::MidiEvent(msg) => {
                if msg.len() == 3 {
                    let msg_type = msg[0] & 0xF0;
                    let ch = msg[0] & 0x0F;
                    let data1 = msg[1];
                    let data2 = msg[2];
                    let ev = match msg_type {
                        0x80 => Some(MidiEvent::NoteOff(ch,data1)),
                        0x90 => Some(MidiEvent::NoteOn(ch,data1,data2)),
                        0xB0 => Some(MidiEvent::ControlChange(ch,data1,data2)),
                        _ => None,
                    };
                    if let Some(ev) = ev { 
                        app.on_midi(ev);
                    }
                }
            }
        }

    }

    // cleanup
    if let Some(mc) = app.midi_connection {
        mc.close();
    }
    disable_raw_mode()?;
    terminal.clear()?;

    Ok(())
}




