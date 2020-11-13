
```
                        midiosch

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
```
    

A small MIDI to OSC brigde written in rust, intended to be liteweight and without dependencies.

`midiosch` stands for "MIDI to OSC Handler", and it is pronounced `/midioʎ/` ( *mi diosh* in spanish, which roughly means: *oh my gosh*)

### Installation

Just download a pre-built binary from [releases](https://github.com/diegodorado/midiosch/releases).

### How to use

Plug your MIDI devices and run the program.  

It runs in a terminal emulator routing (some) midi events to OSC messages to port `9000`.  
Default OSC port, and MIDI device can be passed by argument.  
If no MIDI device is passed, a prompt will ask you which device to use.  
