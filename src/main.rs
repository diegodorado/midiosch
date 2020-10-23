use nannou::prelude::*;
use nannou::ui::prelude::*;
use nannou_osc as osc;
use nannou_osc::Type;
use midir::{MidiInput, Ignore};
use std::io::{stdin, stdout, Write};
use std::error::Error;
use crossbeam::crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use std::thread;
use std::env;
use clap;


fn main() {

    nannou::app(model)
        .update(update)
        .simple_window(view)
        .run();
}

#[derive(Debug)]
pub enum ThreadState{
    Loading,
    SelectPortRequest,
    Running,
}

struct Model {
    ui: Ui,
    ids: Ids,
    resolution: u8,
    thread_state: ThreadState,
    sender: Sender<ThreadState>,
    receiver: Receiver<ThreadState>,
    midi_receiver: Receiver<MidiMsg>,
}

widget_ids! {
    struct Ids {
        resolution,
        scale,
        rotation,
        random_color,
        position,
    }
}

pub enum MidiMsg {
    NoteOn(u8,u8,u8),
    NoteOff(u8,u8),
    ControlChange(u8,u8,u8),
}

fn model(app: &App) -> Model {

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
    let midi_input: Option<u8> = match matches.value_of_t("input") {
        Ok(v) => Some(v),
        Err(_) => None,
    };

    let (sender_t, receiver) = bounded::<ThreadState>(1);
    let (sender, receiver_t) = bounded::<ThreadState>(1);
    let (midi_sender, midi_receiver) = unbounded::<MidiMsg>();

    thread::spawn(move || match comm_thread(port, midi_input, sender_t, receiver_t, midi_sender) {
        Ok(_) => (),
        Err(err) => println!("Error: {}", err),
    });

    // Set the loop mode to wait for events, an energy-efficient option for pure-GUI apps.
    app.set_loop_mode(LoopMode::Wait);

    // Create the UI.
    let mut ui = app.new_ui().build().unwrap();

    // Generate some ids for our widgets.
    let ids = Ids::new(ui.widget_id_generator());

    // Init our variables
    let resolution = 6;

    Model {
        ui,
        ids,
        resolution,
        thread_state: ThreadState::Loading, 
        sender, 
        receiver, 
        midi_receiver
    }

}

fn update(_app: &App, model: &mut Model, _update: Update) {

    // Calling `set_widgets` allows us to instantiate some widgets.
    let ui = &mut model.ui.set_widgets();

    match model.receiver.try_recv() {
        Ok(s) => { model.thread_state = s;}
        _ => {}
    }

    match model.thread_state {
        ThreadState::Loading => {
        },
        ThreadState::SelectPortRequest => {
        },
        ThreadState::Running => {
            let midi_messages: Vec<MidiMsg> = model.midi_receiver.try_iter().collect();
            for m in midi_messages {
                match m {
                    MidiMsg::NoteOn(ch,note,vel) => { println!("note on {} {} {} ", ch,note, vel);}
                    MidiMsg::NoteOff(ch,note) => { println!("note off {} {}", ch,note);}
                    MidiMsg::ControlChange(ch,num,val) => { println!("cc {} {} {}", ch,num,val);}
                }
            }
        }
    }

}

fn view(app: &App, model: &Model, frame: Frame) {

    let draw = app.draw();

    match model.thread_state {
        ThreadState::Loading => {
            draw.background().color(RED);
        },
        ThreadState::SelectPortRequest => {
            draw.background().color(BLUE);
        },
        ThreadState::Running => {
            draw.background().color(PLUM);
        }
    }

    // Write the result of our drawing to the window's frame.
    draw.to_frame(app, &frame).unwrap();

    // Draw the state of the `Ui` to the frame.
    model.ui.draw_to_frame(app, &frame).unwrap();

}



pub fn comm_thread(
        port: u16, 
        midi_input: Option<u8>, 
        sender: Sender<ThreadState>, 
        receiver: Receiver<ThreadState>, 
        midi_sender: Sender<MidiMsg>
    ) -> Result<(), Box<dyn Error>> {

    let mut input = String::new();
    let mut midi_in = MidiInput::new("midir reading input")?;
    midi_in.ignore(Ignore::None);

    // The osc-sender expects a string in the format "address:port", for example "127.0.0.1:1234"
    // "127.0.0.1" is equivalent to your computers internal address.
    let target_addr = format!("{}:{}", "127.0.0.1", port);

    // This is the osc Sender which contains a couple of expectations in case something goes wrong.
    let osc_sender = osc::sender()
        .expect("Could not bind to default socket")
        .connect(target_addr)
        .expect("Could not connect to socket at address");

    // Get an input port (read from console if multiple are available)
    let in_ports = midi_in.ports();

    sender.send(ThreadState::SelectPortRequest).unwrap();

    let in_port = match in_ports.len() {
        0 => return Err("no input port found".into()),
        1 => {
            println!("Choosing the only available input port: {}", midi_in.port_name(&in_ports[0]).unwrap());
            &in_ports[0]
        },
        _ => {
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
    };

    println!("\nOpening connection");

    // _conn_in needs to be a named parameter, because it needs to be kept alive until the end of the scope
    let _conn_in = midi_in.connect(
        in_port,
        "midir-read-input",
        move |_, msg, _| {

            if msg.len() == 3 {
                if msg[0] == 0x90 {
                    midi_sender.send(MidiMsg::NoteOn(0,msg[1],msg[2])).unwrap();
                    let osc_addr = "/circle/position".to_string();
                    let args = vec![Type::Int(0)];
                    let packet = (osc_addr, args);
                    osc_sender.send(packet).ok();
                } else if msg[0] == 0x80 {
                    midi_sender.send(MidiMsg::NoteOff(0,msg[1])).unwrap();
                } else if msg[0] == 0xB0 {
                    midi_sender.send(MidiMsg::ControlChange(0,msg[1],msg[2])).unwrap();
                }
            }

        },
        (),
    )?;

    input.clear();
    stdin().read_line(&mut input)?; // wait for next enter key press

    println!("Closing connection");
    Ok(())
}
