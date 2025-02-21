# Pending Tasks

## Features

- [ ] Implement simple combat system.
- [x] Animate movement of player/objects between their sprites.
- [x] Other players must be rendered with an avatar in the client.
- [x] Monster(s) must change directions when walking.

## Improvements/Refactorings

- [ ] Must decouple `game_objects` from the server.
- [x] Other players must turn direction adequately.
- [x] Implement traits to deserialize/serialize and send through UDP/TCP.

## Bugs

- [ ] Monster pathfinding does not work properly when player moves.
- [ ] Fix moving objects and migrate to UDP instead of TCP.
- [ ] Monster must not retarget players like crazy.
- [ ] Fix diagonal movements for players/monsters (other players are turning fine).
