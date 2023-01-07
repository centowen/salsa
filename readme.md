# Running a debug session

## Prerequisite
The simplest method to serve the frontend is using trunk. To install trunk using cargo run

```
cargo install --locked trunk
```

## Running
Start both backend and trunk using the suppliend scripts under development. First start the backend

```shell
./development/run-backend.sh
```

In a separate window run trunk to serve the frontend

```shell
./development/serve-frontend.sh
```
