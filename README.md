# miata

Learn to drive a manual transmission, in Rust. You sit in a low-poly NA Miata,
find the bite point, pull away, shift and stop without stalling. When you can
pass the lessons here, the sequence and timing carry over to a real car.

**Play in the browser: https://lubabs770.github.io/miata/**

> With a keyboard or gamepad this trains the sequence, the timing and reading
> the revs and engine sound. The physical feel of a clutch pedal only carries
> over with real pedals (wheel/pedal support is planned).

## What's in it

- A drivetrain model with an engine, a clutch that slips and locks, a 5-speed
  gearbox and the car's motion. It stalls, grinds, over-revs and rolls back on
  hills like a real car.
- The bite point moves a little each session, so you learn to *feel* for it
  rather than memorise it.
- A coffee cup on the dash that spills if you drive jerkily.
- Fifteen lessons, each graded with 1–3 stars, plus free drive:
  1. Find the bite point
  2. Pull away
  3. Stop without stalling
  4. Upshift 1→2→3
  5. Slow down and downshift
  6. Drive 500 m
  7. Hill start with the handbrake
  8. Hill start on the foot brake
  9. Car-park creep into a marked space
  10. Stop-and-go traffic behind another car
  11. Stop sign and left turn
  12. Stop sign on a hill
  13. Paddles: shift points
  14. Paddles: engine braking downhill
  15. Paddles: gear for the corner
- A procedural engine sound that follows rpm and load.
- Three cars: the NA Miata, a turbo hot hatch, and an old pickup that stalls
  easily.
- Gearbox modes for free drive: **Manual**, **Auto-clutch** (you pick gears and
  the computer works the clutch) and **Paddles** (+/−, like a modern
  dual-clutch car).
- Cockpit, hood, chase and bird's-eye camera views.
- A low-poly town for free drive: a grid of avenues and cross streets lined
  with buildings.
- Sound effects: gear grind, stall, handbrake ratchet and refused paddle shifts.
- A replay graph after every lesson attempt: clutch, gas, brake and rpm over
  time, with stalls marked, so you can see what your feet did.
- Progress saving: best stars per lesson, plus your car and gearbox choices
  (browser storage on the web, a JSON file on desktop).

Planned next: real streets from OpenStreetMap. See the [design spec](docs/superpowers/specs/2026-10-06-miata-design.md).

## Controls

| | Keyboard | Gamepad |
|---|---|---|
| Gas | W | RT |
| Brake | S | Left stick down |
| Clutch | Left Shift (hold; lets up slowly) | LT |
| Steer | A / D | Left stick |
| Gears | 1–5, R, N | Right stick as an H-pattern, R3 = neutral |
| Handbrake (toggle) | Space | X |
| Ignition | I | Y |
| Camera view (cockpit / hood / chase / bird's-eye) | V | Select |
| Paddle shift down / up (auto-clutch and paddle modes) | Q / E | LB / RB |

The engine only starts with the clutch in (or in neutral), as in most real cars.

Every key and gamepad button can be rebound in **Lessons → Controls…**. Click
a binding and press the new key; if it's already taken, the two actions swap.
Bindings are saved with your progress.

**Steering wheel and pedals:** Controls… → **Wheel & pedals…** runs a short
calibration. You press each pedal and turn the wheel, and it learns which axis
is which and its range, including inverted pedals. An optional progressive
curve gives finer gas and brake control.

## Docs

- [How the physics works](docs/physics.md): the clutch, stalling and tuning knobs
- [Adding a car](docs/adding-a-car.md): cars are TOML files
- [Development](docs/development.md): layout, tests, CI and deploys

## License

MIT
