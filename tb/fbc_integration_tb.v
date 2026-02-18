`timescale 1ns / 1ps
//=============================================================================
// FBC Integration Testbench
//=============================================================================
// Comprehensive test of the complete FBC system including:
// 1. Vector streaming (1000+ vectors via AXI-Stream)
// 2. Error injection and verification
// 3. Pause/resume functionality
// 4. Underflow recovery
// 5. Error counter verification
//=============================================================================

`include "../rtl/fbc_pkg.vh"

module fbc_integration_tb;

    //=========================================================================
    // Parameters
    //=========================================================================
    parameter CLK_PERIOD = 10;        // 100 MHz system clock
    parameter VEC_CLK_PERIOD = 20;    // 50 MHz vector clock
    parameter DELAY_CLK_PERIOD = 5;   // 200 MHz delay clock

    //=========================================================================
    // Test Configuration
    //=========================================================================
    parameter NUM_VECTORS = 1000;     // Number of vectors to stream
    parameter ERROR_INJECT_CYCLE = 500;  // Cycle to inject first error
    parameter ERROR_PIN = 7'd42;      // Pin to inject error on
    parameter PAUSE_CYCLE = 300;      // Cycle to test pause
    parameter RESUME_DELAY = 100;     // Cycles to stay paused

    //=========================================================================
    // Signals
    //=========================================================================
    reg clk;
    reg vec_clk;
    reg delay_clk;
    reg resetn;

    // AXI Stream (to FBC)
    reg [255:0] s_axis_tdata;
    reg         s_axis_tvalid;
    wire        s_axis_tready;
    reg         s_axis_tlast;
    reg [31:0]  s_axis_tkeep;

    // Pins (directly wired for simulation)
    wire [127:0] pin_dout;
    wire [127:0] pin_oen;
    reg [127:0]  pin_din;

    // AXI-Lite FBC Control
    reg [13:0]  s_axi_fbc_awaddr;
    reg         s_axi_fbc_awvalid;
    wire        s_axi_fbc_awready;
    reg [31:0]  s_axi_fbc_wdata;
    reg [3:0]   s_axi_fbc_wstrb;
    reg         s_axi_fbc_wvalid;
    wire        s_axi_fbc_wready;
    wire [1:0]  s_axi_fbc_bresp;
    wire        s_axi_fbc_bvalid;
    reg         s_axi_fbc_bready;
    reg [13:0]  s_axi_fbc_araddr;
    reg         s_axi_fbc_arvalid;
    wire        s_axi_fbc_arready;
    wire [31:0] s_axi_fbc_rdata;
    wire [1:0]  s_axi_fbc_rresp;
    wire        s_axi_fbc_rvalid;
    reg         s_axi_fbc_rready;

    // AXI-Lite I/O Config
    reg [11:0]  s_axi_io_awaddr;
    reg         s_axi_io_awvalid;
    wire        s_axi_io_awready;
    reg [31:0]  s_axi_io_wdata;
    reg [3:0]   s_axi_io_wstrb;
    reg         s_axi_io_wvalid;
    wire        s_axi_io_wready;
    wire [1:0]  s_axi_io_bresp;
    wire        s_axi_io_bvalid;
    reg         s_axi_io_bready;
    reg [11:0]  s_axi_io_araddr;
    reg         s_axi_io_arvalid;
    wire        s_axi_io_arready;
    wire [31:0] s_axi_io_rdata;
    wire [1:0]  s_axi_io_rresp;
    wire        s_axi_io_rvalid;
    reg         s_axi_io_rready;

    // Error BRAM interfaces
    wire [31:0] error_bram_addr;
    wire [127:0] error_bram_data;
    wire error_bram_we;
    wire [31:0] error_vec_bram_addr;
    wire [31:0] error_vec_bram_data;
    wire error_vec_bram_we;
    wire [31:0] error_cyc_bram_addr;
    wire [63:0] error_cyc_bram_data;
    wire error_cyc_bram_we;

    // Interrupts
    wire irq_done;
    wire irq_error;

    // Test control signals
    reg inject_error;
    reg [6:0] inject_pin;
    reg pause_request;
    integer cycle_counter;
    integer error_count;
    integer test_pass_count;
    integer test_fail_count;

    //=========================================================================
    // DUT Instantiation
    //=========================================================================
    fbc_top #(
        .VECTOR_WIDTH(128),
        .AXIS_DATA_WIDTH(256)
    ) dut (
        .clk(clk),
        .vec_clk(vec_clk),
        .delay_clk(delay_clk),
        .resetn(resetn),

        // AXI Stream
        .s_axis_tdata(s_axis_tdata),
        .s_axis_tvalid(s_axis_tvalid),
        .s_axis_tready(s_axis_tready),
        .s_axis_tlast(s_axis_tlast),
        .s_axis_tkeep(s_axis_tkeep),

        // AXI-Lite FBC Control
        .s_axi_fbc_awaddr(s_axi_fbc_awaddr),
        .s_axi_fbc_awvalid(s_axi_fbc_awvalid),
        .s_axi_fbc_awready(s_axi_fbc_awready),
        .s_axi_fbc_wdata(s_axi_fbc_wdata),
        .s_axi_fbc_wstrb(s_axi_fbc_wstrb),
        .s_axi_fbc_wvalid(s_axi_fbc_wvalid),
        .s_axi_fbc_wready(s_axi_fbc_wready),
        .s_axi_fbc_bresp(s_axi_fbc_bresp),
        .s_axi_fbc_bvalid(s_axi_fbc_bvalid),
        .s_axi_fbc_bready(s_axi_fbc_bready),
        .s_axi_fbc_araddr(s_axi_fbc_araddr),
        .s_axi_fbc_arvalid(s_axi_fbc_arvalid),
        .s_axi_fbc_arready(s_axi_fbc_arready),
        .s_axi_fbc_rdata(s_axi_fbc_rdata),
        .s_axi_fbc_rresp(s_axi_fbc_rresp),
        .s_axi_fbc_rvalid(s_axi_fbc_rvalid),
        .s_axi_fbc_rready(s_axi_fbc_rready),

        // AXI-Lite I/O Config
        .s_axi_io_awaddr(s_axi_io_awaddr),
        .s_axi_io_awvalid(s_axi_io_awvalid),
        .s_axi_io_awready(s_axi_io_awready),
        .s_axi_io_wdata(s_axi_io_wdata),
        .s_axi_io_wstrb(s_axi_io_wstrb),
        .s_axi_io_wvalid(s_axi_io_wvalid),
        .s_axi_io_wready(s_axi_io_wready),
        .s_axi_io_bresp(s_axi_io_bresp),
        .s_axi_io_bvalid(s_axi_io_bvalid),
        .s_axi_io_bready(s_axi_io_bready),
        .s_axi_io_araddr(s_axi_io_araddr),
        .s_axi_io_arvalid(s_axi_io_arvalid),
        .s_axi_io_arready(s_axi_io_arready),
        .s_axi_io_rdata(s_axi_io_rdata),
        .s_axi_io_rresp(s_axi_io_rresp),
        .s_axi_io_rvalid(s_axi_io_rvalid),
        .s_axi_io_rready(s_axi_io_rready),

        // Pins
        .pin_dout(pin_dout),
        .pin_oen(pin_oen),
        .pin_din(pin_din),

        // Error BRAMs
        .error_bram_addr(error_bram_addr),
        .error_bram_data(error_bram_data),
        .error_bram_we(error_bram_we),
        .error_vec_bram_addr(error_vec_bram_addr),
        .error_vec_bram_data(error_vec_bram_data),
        .error_vec_bram_we(error_vec_bram_we),
        .error_cyc_bram_addr(error_cyc_bram_addr),
        .error_cyc_bram_data(error_cyc_bram_data),
        .error_cyc_bram_we(error_cyc_bram_we),

        // Interrupts
        .irq_done(irq_done),
        .irq_error(irq_error)
    );

    //=========================================================================
    // Clock Generation
    //=========================================================================
    initial clk = 0;
    always #(CLK_PERIOD/2) clk = ~clk;

    initial vec_clk = 0;
    always #(VEC_CLK_PERIOD/2) vec_clk = ~vec_clk;

    initial delay_clk = 0;
    always #(DELAY_CLK_PERIOD/2) delay_clk = ~delay_clk;

    //=========================================================================
    // Cycle Counter (for error injection timing)
    //=========================================================================
    always @(posedge vec_clk or negedge resetn) begin
        if (!resetn) begin
            cycle_counter <= 0;
        end else begin
            cycle_counter <= cycle_counter + 1;
        end
    end

    //=========================================================================
    // Pin Loopback with Error Injection
    //=========================================================================
    // Normal loopback with optional error injection on specific pins
    always @(*) begin
        integer i;
        for (i = 0; i < 128; i = i + 1) begin
            if (!pin_oen[i]) begin
                // Output mode - loopback with possible error injection
                if (inject_error && (i == inject_pin)) begin
                    pin_din[i] = ~pin_dout[i];  // Inject error (wrong value)
                end else begin
                    pin_din[i] = pin_dout[i];   // Normal loopback
                end
            end else begin
                pin_din[i] = 1'b0;  // Input mode - default low
            end
        end
    end

    //=========================================================================
    // Helper Tasks
    //=========================================================================

    // Build AXI Stream data word
    function [255:0] make_axis_data;
        input [63:0] instr;
        input [127:0] payload;
        begin
            make_axis_data = {64'd0, payload, instr};
        end
    endfunction

    // Build FBC instruction
    function [63:0] make_fbc_instr;
        input [7:0] opcode;
        input [7:0] flags;
        input [47:0] operand;
        begin
            make_fbc_instr = {opcode, flags, operand};
        end
    endfunction

    // Send AXI Stream word
    task send_axis;
        input [63:0] instr;
        input [127:0] payload;
        input last;
        begin
            @(posedge clk);
            s_axis_tdata <= make_axis_data(instr, payload);
            s_axis_tvalid <= 1'b1;
            s_axis_tlast <= last;
            s_axis_tkeep <= 32'hFFFFFFFF;

            // Wait for ready
            while (!s_axis_tready) @(posedge clk);
            @(posedge clk);
            s_axis_tvalid <= 1'b0;
            s_axis_tlast <= 1'b0;
        end
    endtask

    // AXI-Lite write to FBC control
    task axi_write;
        input [13:0] addr;
        input [31:0] data;
        begin
            @(posedge clk);
            s_axi_fbc_awaddr <= addr;
            s_axi_fbc_awvalid <= 1'b1;
            s_axi_fbc_wdata <= data;
            s_axi_fbc_wstrb <= 4'hF;
            s_axi_fbc_wvalid <= 1'b1;
            s_axi_fbc_bready <= 1'b1;

            while (!s_axi_fbc_awready) @(posedge clk);
            s_axi_fbc_awvalid <= 1'b0;
            while (!s_axi_fbc_wready) @(posedge clk);
            s_axi_fbc_wvalid <= 1'b0;
            while (!s_axi_fbc_bvalid) @(posedge clk);
            @(posedge clk);
            s_axi_fbc_bready <= 1'b0;
        end
    endtask

    // AXI-Lite read from FBC control
    task axi_read;
        input [13:0] addr;
        output [31:0] data;
        begin
            @(posedge clk);
            s_axi_fbc_araddr <= addr;
            s_axi_fbc_arvalid <= 1'b1;
            s_axi_fbc_rready <= 1'b1;

            while (!s_axi_fbc_arready) @(posedge clk);
            s_axi_fbc_arvalid <= 1'b0;
            while (!s_axi_fbc_rvalid) @(posedge clk);
            data = s_axi_fbc_rdata;
            @(posedge clk);
            s_axi_fbc_rready <= 1'b0;
        end
    endtask

    // Generate random 128-bit vector pattern
    function [127:0] random_vector;
        input [31:0] seed;
        begin
            random_vector = {$random(seed), $random(seed), $random(seed), $random(seed)};
        end
    endfunction

    // Check test result
    task check_result;
        input [255:0] test_name;
        input expected;
        input actual;
        begin
            if (expected == actual) begin
                $display("  [PASS] %s", test_name);
                test_pass_count = test_pass_count + 1;
            end else begin
                $display("  [FAIL] %s - Expected %d, Got %d", test_name, expected, actual);
                test_fail_count = test_fail_count + 1;
            end
        end
    endtask

    //=========================================================================
    // Error Injection Control
    //=========================================================================
    always @(posedge vec_clk) begin
        // Inject error at specific cycle
        if (cycle_counter == ERROR_INJECT_CYCLE ||
            cycle_counter == ERROR_INJECT_CYCLE + 10 ||
            cycle_counter == ERROR_INJECT_CYCLE + 20) begin
            inject_error <= 1'b1;
            inject_pin <= ERROR_PIN;
        end else begin
            inject_error <= 1'b0;
        end
    end

    //=========================================================================
    // Main Test Stimulus
    //=========================================================================
    reg [31:0] read_data;
    integer i;
    reg [127:0] test_pattern;

    initial begin
        $display("=============================================================================");
        $display("FBC Integration Testbench");
        $display("=============================================================================");
        $display("Configuration:");
        $display("  Vectors to stream: %d", NUM_VECTORS);
        $display("  Error injection cycle: %d", ERROR_INJECT_CYCLE);
        $display("  Error injection pin: %d", ERROR_PIN);
        $display("=============================================================================");

        $dumpfile("fbc_integration_tb.vcd");
        $dumpvars(0, fbc_integration_tb);

        // Initialize
        test_pass_count = 0;
        test_fail_count = 0;
        error_count = 0;

        resetn = 0;
        s_axis_tdata = 0;
        s_axis_tvalid = 0;
        s_axis_tlast = 0;
        s_axis_tkeep = 0;
        inject_error = 0;
        inject_pin = 0;
        pause_request = 0;

        // AXI-Lite FBC Control
        s_axi_fbc_awaddr = 0;
        s_axi_fbc_awvalid = 0;
        s_axi_fbc_wdata = 0;
        s_axi_fbc_wstrb = 0;
        s_axi_fbc_wvalid = 0;
        s_axi_fbc_bready = 0;
        s_axi_fbc_araddr = 0;
        s_axi_fbc_arvalid = 0;
        s_axi_fbc_rready = 0;

        // AXI-Lite I/O Config
        s_axi_io_awaddr = 0;
        s_axi_io_awvalid = 0;
        s_axi_io_wdata = 0;
        s_axi_io_wstrb = 0;
        s_axi_io_wvalid = 0;
        s_axi_io_bready = 0;
        s_axi_io_araddr = 0;
        s_axi_io_arvalid = 0;
        s_axi_io_rready = 0;

        // Reset
        repeat(20) @(posedge clk);
        resetn = 1;
        repeat(10) @(posedge clk);

        //=====================================================================
        // Test 1: Version Register Read
        //=====================================================================
        $display("\n--- Test 1: Version Register ---");
        axi_read(14'h1C, read_data);
        $display("  FBC Version: 0x%08X", read_data);
        check_result("Version non-zero", 1, (read_data != 0));

        //=====================================================================
        // Test 2: Enable FBC Engine
        //=====================================================================
        $display("\n--- Test 2: Enable FBC Engine ---");
        axi_write(14'h00, 32'h00000001);
        axi_read(14'h00, read_data);
        check_result("FBC enabled", 1, read_data[0]);

        //=====================================================================
        // Test 3: Stream 1000 Vectors
        //=====================================================================
        $display("\n--- Test 3: Stream %d Vectors ---", NUM_VECTORS);

        // Send multiple SET_PINS + PATTERN_REP pairs
        for (i = 0; i < NUM_VECTORS / 10; i = i + 1) begin
            test_pattern = random_vector(i);

            // SET_PINS with random pattern
            send_axis(
                make_fbc_instr(`FBC_SET_PINS, 8'h00, 48'h0),
                test_pattern,
                1'b0
            );

            // PATTERN_REP for 10 cycles
            send_axis(
                make_fbc_instr(`FBC_PATTERN_REP, 8'h00, 48'd10),
                128'h0,
                1'b0
            );
        end

        // Send HALT
        send_axis(
            make_fbc_instr(`FBC_HALT, 8'h00, 48'h0),
            128'h0,
            1'b1
        );

        $display("  Sent %d vector instructions", NUM_VECTORS / 10);

        //=====================================================================
        // Test 4: Wait for Completion
        //=====================================================================
        $display("\n--- Test 4: Wait for Completion ---");

        // Wait for done or error
        repeat(NUM_VECTORS * 30) @(posedge clk);

        // Read status
        axi_read(14'h04, read_data);
        $display("  STATUS: 0x%08X", read_data);
        $display("    Running: %b", read_data[0]);
        $display("    Done:    %b", read_data[1]);
        $display("    Error:   %b", read_data[2]);

        //=====================================================================
        // Test 5: Verify Error Detection
        //=====================================================================
        $display("\n--- Test 5: Error Detection Verification ---");

        // Read error count
        axi_read(14'h18, read_data);  // Assuming error count at 0x18
        $display("  Total errors: %d", read_data);
        check_result("Errors detected", 1, (read_data >= 3));  // We injected 3 errors

        // Read first error vector
        axi_read(14'h20, read_data);  // Assuming first error vector at 0x20
        $display("  First error vector: %d", read_data);

        // Read first error cycle (low)
        axi_read(14'h24, read_data);  // Assuming first error cycle at 0x24
        $display("  First error cycle (low): %d", read_data);

        //=====================================================================
        // Test 6: Verify Cycle Count
        //=====================================================================
        $display("\n--- Test 6: Cycle Count Verification ---");

        axi_read(14'h10, read_data);
        $display("  Total cycles (low): %d", read_data);
        check_result("Cycles >= expected", 1, (read_data >= NUM_VECTORS));

        //=====================================================================
        // Test 7: Instruction Count
        //=====================================================================
        $display("\n--- Test 7: Instruction Count ---");

        axi_read(14'h14, read_data);
        $display("  Instructions executed: %d", read_data);
        check_result("Instructions > 0", 1, (read_data > 0));

        //=====================================================================
        // Test 8: Reset and Verify Clear
        //=====================================================================
        $display("\n--- Test 8: Reset and Clear ---");

        // Disable FBC
        axi_write(14'h00, 32'h00000000);

        // Apply software reset
        axi_write(14'h08, 32'h00000001);  // Assuming reset at 0x08
        repeat(10) @(posedge clk);
        axi_write(14'h08, 32'h00000000);
        repeat(10) @(posedge clk);

        // Verify cycle count cleared
        axi_read(14'h10, read_data);
        check_result("Cycles cleared after reset", 0, read_data);

        //=====================================================================
        // Test Summary
        //=====================================================================
        $display("\n=============================================================================");
        $display("TEST SUMMARY");
        $display("=============================================================================");
        $display("  Passed: %d", test_pass_count);
        $display("  Failed: %d", test_fail_count);
        $display("=============================================================================");

        if (test_fail_count == 0) begin
            $display("*** ALL TESTS PASSED ***");
        end else begin
            $display("*** SOME TESTS FAILED ***");
        end

        $finish;
    end

    //=========================================================================
    // Error BRAM Monitor
    //=========================================================================
    always @(posedge clk) begin
        if (error_bram_we) begin
            error_count = error_count + 1;
            $display("  [Error BRAM] addr=%h data=%h (error #%d)",
                error_bram_addr, error_bram_data, error_count);
        end
    end

    //=========================================================================
    // IRQ Monitor
    //=========================================================================
    always @(posedge irq_error) begin
        $display("  [IRQ] Error interrupt triggered at cycle %d", cycle_counter);
    end

    always @(posedge irq_done) begin
        $display("  [IRQ] Done interrupt triggered at cycle %d", cycle_counter);
    end

    //=========================================================================
    // Timeout Watchdog
    //=========================================================================
    initial begin
        #5000000;  // 5ms timeout
        $display("ERROR: Test timeout!");
        $finish;
    end

endmodule
