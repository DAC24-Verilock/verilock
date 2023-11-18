<h1 align="center">
  verilock
</h1>
<p align="center">
  <img src="./img/verilock logo.png" width="160" />
</p>
`verilock` is a verification tool that can detect deadlocks in asynchronous circuits specified in SystemVerilog.
This repository includes the artifacts for the DAC 2024 submission #503.

- `resource/cases` folder includes the cases collected from literatures.
- `resource/gen` folder includes the circuits synthesized randomly by [xin](https://github.com/DAC24-Verilock/xin), where `Gen1` ~ `Gen5` are the cases without deadlocks, and `Gen6` ~ `Gen10` are the ones with deadlocks.

### Installation

1. Install Rust tool chain.
```shell
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
2. Clone this repository.

3. Navigate to the project directory.
```shell
cd verilock
```
4. Build the project.
```shell
cargo build
```

### Reproducing Experimental Results
To replicate the experimental result from DAC24 submission #503, please adhere to these instructions.
#### Observing Verification Results
1. To observe the results that `verilock` verifies the cases from the literatures, run the command: 
```shell
cargo run -- RQ1
```

2. To observe the results that `verilock` verifies the cases randomly synthesized, run the command:
```shell
cargo run -- RQ2
```

#### Running Performance Benchmark
`verilock` uses [criterion](https://docs.rs/criterion/latest/criterion/) to microbenchmark the performance.
Run the following command to benchmark.
```shell
cargo bench
```
### Raw Data
The execution time reports for the experiments can be accessed online through the following link: [execution time](https://dac24-verilock.github.io/verilock/report/index).

### Caveats
This prototype serves research purposes and currently supports only a subset of the SystemVerilog syntax.
1. ❌ NonANSI-style modules and interfaces.
2. ❌ multiple module instantiations in a single line.
3. ❌ named port binding.
4. ❌ nested module and interface declarations.
5. ❌ non-blocking assignments.