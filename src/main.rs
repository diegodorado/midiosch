use nannou_osc as osc;
use nannou_osc::Type;
use midir::{MidiInput, Ignore};
use clap;

use std::io;
use tui::Terminal;
use tui::backend::CrosstermBackend;

use tui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Gauge, Sparkline},
};

use crossterm::{
    event::{self, EnableMouseCapture, Event as CEvent, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};

use std::{
    error::Error,
    io::{stdin, stdout, Write},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

enum Event<I> {
    Input(I),
    Tick,
    MidiEvent(Vec<u8>),
}

struct App {
    progress: u16,
    notes: Vec<u64>,
    cc: Vec<u64>,
}

impl App {
    fn new() -> App {
        let mut notes = vec![0];
        let mut cc = vec![0];
        App {
            progress: 0,
            notes,
            cc,
        }
    }

    fn update(&mut self) {
        if(self.notes.len() >=200){
            self.notes.pop();
        }
        self.notes.insert(0, 1);
    }

    fn note_on(&mut self, ch: u8, note: u8, vel: u8) {
        if(self.notes.len() >=200){
            self.notes.pop();
        }
        self.notes.insert(0, note.into());
    }


}


fn main() -> Result<(), Box<dyn Error>> {

    let matches = clap::App::new("My Super Program")
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

    let mut midi_in = MidiInput::new("midir reading input")?;
    midi_in.ignore(Ignore::None);

    let target_addr = format!("{}:{}", "127.0.0.1", port);
    let osc_sender = osc::sender()
        .expect("Could not bind to default socket")
        .connect(target_addr)
        .expect("Could not connect to socket at address");

    // Get an input port (read from console if multiple are available)
    let in_ports = midi_in.ports();

    let in_port = match in_ports.len() {
        0 => return Err("no input port found".into()),
        _ => {
            match midi_input {
                Some(index) => {
                    if(index < in_ports.len()){
                        println!("Choosing the only available input port: {}", midi_in.port_name(&in_ports[0]).unwrap());
                        &in_ports[index]
                    }
                    else{
                        return Err("input port out of range".into());
                    }
                }
                None => {
                    if(in_ports.len()==1){
                        println!("Choosing the only available input port: {}", midi_in.port_name(&in_ports[0]).unwrap());
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

    enable_raw_mode()?;
    // Terminal initialization
    let mut stdout = stdout();

    execute!(stdout, EnableMouseCapture)?;


    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Setup input handling
    let (tx, rx) = mpsc::channel();

    let tx2 = tx.clone();
    // _conn_in needs to be a named parameter, because it needs to be kept alive until the end of the scope
    let _conn_in = midi_in.connect(
        in_port,
        "midir-read-input",
        move |_, msg, _| {
            tx2.send(Event::MidiEvent(msg.to_vec())).unwrap();
        },
        (),
    )?;

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

    let mut app = App::new();

    terminal.clear()?;

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints(
                    [
                        Constraint::Percentage(25),
                        Constraint::Percentage(25),
                        Constraint::Length(3),
                        Constraint::Length(3),
                        Constraint::Length(7),
                        Constraint::Min(0),
                    ]
                    .as_ref(),
                )
                .split(f.size());

            let sparkline = Sparkline::default()
                .block(
                    Block::default()
                        .title("NOTES")
                        .borders(Borders::LEFT | Borders::RIGHT),
                )
                .data(&app.notes)
                .max(127)
                .style(Style::default().fg(Color::Yellow));
            f.render_widget(sparkline, chunks[0]);

        })?;

        match rx.recv()? {
            Event::Input(event) => match event.code {
                KeyCode::Char('q') => {
                    terminal.clear()?;
                    break;
                }
                _ => {}
            },
            Event::Tick => {
                app.update();
            },
            Event::MidiEvent(msg) => {
                if msg.len() == 3 {
                    if msg[0] == 0x90 {
                        let ch = msg[0];
                        let note = msg[1];
                        let vel = msg[2];
                        let addr = format!("/note/{}/{}", ch, note);
                        let args = vec![Type::Int(vel.into())];
                        let packet = (addr, args);
                        osc_sender.send(packet).ok();
                        app.note_on(ch, note, vel);
                    } else if msg[0] == 0x80 {
                    } else if msg[0] == 0xB0 {
                    }
                }

            }
        }

    }

    disable_raw_mode();

    Ok(())
}




