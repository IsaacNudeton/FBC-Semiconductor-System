`timescale 1ns / 1ps
//=============================================================================
// FBC Top-Level Testbench
//=============================================================================
// Tests full system: AXI Stream → FBC Decoder → Vector Engine → Pins
//=============================================================================

`include "../rtl/fbc_pkg.vh"

module fbc_top_tb;

    //=========================================================================
    // Parameters
    //=========================================================================
    parameter CLK_PERIOD = 10;        // 100 MHz
    parameter VEC_CLK_PERIOD = 20;    // 50 MHz
    parameter DELAY_CLK_PERIOD = 5;   // 200 MHz

    //=========================================================================
    // Signals
    //=========================================================================
    reg clk;
    reg vec_clk;
    reg delay_clk;
    reg resetn;

    // AXI Stream
    reg [255:0] s_axis_tdata;
    reg         s_axis_tvalid;
    wire        s_axis_tready;
    reg         s_axis_tlast;
    reg [31:0]  s_axis_tkeep;

    // Pins (directly wired for simulation)
    wire [127:0] pin_dout;
    wire [127:0] pin_oen;
    reg [127:0]  pin_din;

    // AXI-Lite I/O Config (directly drive defaults)
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

    // Interrupts
    wire irq_done;
    wire irq_error;

    //=========================================================================
    // DUT
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

        // Error BRAMs (not connected in sim)
        .error_bram_addr(),
        .error_bram_data(),
        .error_bram_we(),
        .error_vec_bram_addr(),
        .error_vec_bram_data(),
        .error_vec_bram_we(),
        .error_cyc_bram_addr(),
        .error_cyc_bram_data(),
        .error_cyc_bram_we(),

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
    // Pin Loopback (pins echo back what's driven)
    //=========================================================================
    always @(*) begin
        integer i;
        for (i = 0; i < 128; i = i + 1) begin
            if (!pin_oen[i])  // If output enabled
                pin_din[i] = pin_dout[i];  // Loopback
            else
                pin_din[i] = 1'b0;  // Default low for inputs
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

    // AXI-Lite write
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

            // Wait for handshakes
            while (!s_axi_fbc_awready) @(posedge clk);
            s_axi_fbc_awvalid <= 1'b0;
            while (!s_axi_fbc_wready) @(posedge clk);
            s_axi_fbc_wvalid <= 1'b0;
            while (!s_axi_fbc_bvalid) @(posedge clk);
            @(posedge clk);
            s_axi_fbc_bready <= 1'b0;
        end
    endtask

    // AXI-Lite read
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

    //=========================================================================
    // Test Stimulus
    //=========================================================================
    reg [31:0] read_data;

    initial begin
        $display("=== FBC Top-Level Testbench ===");
        $dumpfile("fbc_top_tb.vcd");
        $dumpvars(0, fbc_top_tb);

        // Initialize
        resetn = 0;
        s_axis_tdata = 0;
        s_axis_tvalid = 0;
        s_axis_tlast = 0;
        s_axis_tkeep = 0;

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

        // AXI-Lite I/O Config (tie off - defaults to all BIDI)
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
        // Test 1: Read Version
        //=====================================================================
        $display("\n--- Test 1: Read Version ---");
        axi_read(14'h1C, read_data);
        $display("FBC Version: 0x%08X", read_data);

        //=====================================================================
        // Test 2: Enable FBC
        //=====================================================================
        $display("\n--- Test 2: Enable FBC ---");
        axi_write(14'h00, 32'h00000001);  // Enable
        axi_read(14'h00, read_data);
        $display("CTRL: 0x%08X (expected 0x01)", read_data);

        //=====================================================================
        // Test 3: Send FBC Program
        //=====================================================================
        $display("\n--- Test 3: FBC Program Execution ---");

        // SET_PINS: Set all pins to 0xAA pattern
        $display("Sending SET_PINS...");
        send_axis(
            make_fbc_instr(`FBC_SET_PINS, 8'h00, 48'h0),
            128'hAAAAAAAA_AAAAAAAA_AAAAAAAA_AAAAAAAA,
            1'b0
        );

        // PATTERN_REP: Repeat 1000 times
        $display("Sending PATTERN_REP (1000 cycles)...");
        send_axis(
            make_fbc_instr(`FBC_PATTERN_REP, 8'h00, 48'd1000),
            128'h0,
            1'b0
        );

        // SET_PINS: Change to 0x55 pattern
        $display("Sending SET_PINS (0x55)...");
        send_axis(
            make_fbc_instr(`FBC_SET_PINS, 8'h00, 48'h0),
            128'h55555555_55555555_55555555_55555555,
            1'b0
        );

        // PATTERN_REP: Repeat 500 times
        $display("Sending PATTERN_REP (500 cycles)...");
        send_axis(
            make_fbc_instr(`FBC_PATTERN_REP, 8'h00, 48'd500),
            128'h0,
            1'b0
        );

        // HALT
        $display("Sending HALT...");
        send_axis(
            make_fbc_instr(`FBC_HALT, 8'h00, 48'h0),
            128'h0,
            1'b1  // Last
        );

        //=====================================================================
        // Wait for completion
        //=====================================================================
        $display("\n--- Waiting for completion ---");
        repeat(5000) @(posedge clk);

        // Check status
        axi_read(14'h04, read_data);
        $display("STATUS: 0x%08X", read_data);
        $display("  Running: %b", read_data[0]);
        $display("  Done:    %b", read_data[1]);
        $display("  Error:   %b", read_data[2]);

        // Check cycle count
        axi_read(14'h10, read_data);
        $display("Cycles (low): %d", read_data);

        //=====================================================================
        // Summary
        //=====================================================================
        $display("\n=== Test Summary ===");
        $display("Total instructions: 5");
        $display("Total cycles: ~1502 (1 + 1000 + 1 + 500)");
        $display("Compression: ~300:1");

        if (irq_error) begin
            $display("\n*** TEST FAILED - Error detected ***");
        end else begin
            $display("\n*** TEST PASSED ***");
        end

        $finish;
    end

    //=========================================================================
    // Timeout
    //=========================================================================
    initial begin
        #500000;
        $display("ERROR: Timeout!");
        $finish;
    end

endmodule
