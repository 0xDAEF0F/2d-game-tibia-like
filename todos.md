# Pending Tasks

## Features

- [ ] Implement simple combat system.
- [x] Never spawn a player in the same spot as somebody else.
- [x] Animate movement of player/objects between their sprites.
- [x] Other players must be rendered with an avatar in the client.
- [x] Monster(s) must change directions when walking.

## Improvements/Refactorings

- [ ] Change the cursor to a hand that shows the user is moving something when dragging.
- [ ] Must decouple `game_objects` from the server.
- [ ] We need to verify server side if the player moved the object and was adjacent to it.
- [x] Only be able to move objects if you are adjacent to them.
- [x] Display other users' names above their heads.
- [x] Other players must turn direction adequately.
- [x] Implement traits to deserialize/serialize and send through UDP/TCP.

## Bugs

- [ ] Monster must not retarget players like crazy.
- [ ] Fix diagonal movements for players/monsters (other players are turning fine).
- [ ] Monster pathfinding does not work properly when player moves.
- [x] Fix moving objects and migrate to UDP instead of TCP.
