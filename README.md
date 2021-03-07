
## Unnamed RTS game
This is a hobby project that is a continuation of my 3D graphical programming experiments. This project is more focused and less open ended and also draws from what I've learned building [wgpu-render-node](https://github.com/Nehliin/wgpu-render-node/blob/master/Cargo.toml) and [smol-engine](https://github.com/Nehliin/smol-engine). It's currently not even close to a game.


Current goals:
- [x] Multi-threaded rendering using wgpu
  - [ ] Pbr rendering (without IBL)
  - [x] Gltf model support (WIP working)
- [x] Ui support using egui
- [ ] Animations
- [ ] RTS game mechanics
- [ ] Map editor
-  Basic Online multiplayer support is in progress using laminar. Ambition and quality of this goal depends amount of time I spend on this project. 
   - [x] Server owns the player state and updates client at ~30hz
   - [x] Client(s) send actions which are handled by the server (like move command) 
   - [ ] Win conditions
   - [ ] Game time
   - [ ] Scale to large number of units
   - [ ] New unit creation  
   - ...and a lot more obviously 

### Screenshot (a bit outdated)
Not much to look at currently but feels like all graphical repos should have at least a one screenshot
![Alt text](rts.png?raw=true "A screenshot")