use nannou_osc as osc;
use nannou_osc::Type;
use nannou_osc::Sender;
use nannou_osc::Connected;
use midir::{MidiInput, MidiInputConnection, Ignore};
use clap;


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
    events: Vec<(MidiEvent,Vec<String>)>,
}

impl App {
    fn setup(midi_input_port_name: String, osc_sender: Sender<Connected>) -> App {
        App {
            midi_input_port_name,
            midi_connection: None,
            osc_sender,
            events: Vec::new(),
        }
    }

    fn update(&mut self) {
        if self.events.len() >=100 {
            self.events.truncate(100);
        }
    }

    fn on_midi(&mut self, ev: MidiEvent) {

        let addr;
        let mut args = Vec::new();

        match ev {
            MidiEvent::NoteOff(ch,note) => {
                addr = format!("/note/{}/{}", ch, note);
                args.push(Type::Int(0));
            }
            MidiEvent::NoteOn(ch,note,vel) => {
                addr = format!("/note/{}/{}", ch, note);
                args.push(Type::Int(vel.into()));
            }
            MidiEvent::ControlChange(ch,num,val) => {
                addr = format!("/cc/{}/{}", ch, num);
                args.push(Type::Int(val.into()));
            }
        };

        let packet = (addr.clone(), args);
        self.osc_sender.send(packet).ok();

        let mut row = match ev {
            MidiEvent::NoteOff(ch,note) => vec![ "NOTE".to_string(), ch.to_string(), note.to_string(),"-".to_string()],
            MidiEvent::NoteOn(ch,note,vel) => vec!["NOTE".to_string(), ch.to_string(), note.to_string(), vel.to_string()],
            MidiEvent::ControlChange(ch,num,val) => vec![ "CC".to_string(), ch.to_string(), num.to_string(), val.to_string()],
        };
        row.push(addr);
        self.events.insert(0, (ev,row));

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
        .get_matches();

    let port: u16 = matches.value_of_t("port").unwrap_or(9000);
    let midi_input: Option<usize> = match matches.value_of_t("input") {
        Ok(v) => Some(v),
        Err(_) => None,
    };

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
                    if index < in_ports.len() {
                        &in_ports[index]
                    }
                    else{
                        return Err("input port out of range".into());
                    }
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


    let portname =  format!("{}", midi_in.port_name(&in_ports[0]).unwrap());

    let mut app = App::setup(portname, osc_sender);

    // Setup input handling
    let (tx, rx) = unbounded();

    {
        let tx = tx.clone();
        let tick_rate = Duration::from_millis(200);
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

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(f.size());

            let create_block = |title| {
                Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled(title, Style::default()))
            };
            let paragraph = Paragraph::new(left_text.clone())
                .block(create_block("MIDIOSCH"))
                .alignment(Alignment::Center);
            f.render_widget(paragraph, chunks[0]);

            let style = Style::default();
            let rows = app
                .events
                .iter()
                .map(|(_ev,row)|  Row::StyledData(row.iter(), style));

            let header = ["TYPE","CHAN", "DATA1", "DATA2", "ADDR"];
            let table = Table::new(header.iter(), rows)
                .block(Block::default().borders(Borders::ALL).title("EVENTS"))
                .widths(&[
                    Constraint::Length(6),
                    Constraint::Length(4),
                    Constraint::Length(5),
                    Constraint::Length(5),
                    Constraint::Min(20),
                ]);

            f.render_widget(table, chunks[1]);

        })?;


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
    terminal.clear()?;
    disable_raw_mode()?;
    println!("See you!\n\n{}",logo);

    Ok(())
}




