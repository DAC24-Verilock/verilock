//-------------------------------------------------------------------------------------------------
//  Written by Arash Saifhashemi, saifhash@usc.edu
//  SystemVerilogCSP: Channel interface for modeling channel based asynchronous circuits
//  USC Asynchronous CAD/VLSI Group
//  University of Southern California
//  http://async.usc.edu
//-------------------------------------------------------------------------------------------------

`timescale 1ns/1fs

//-------------------------------------------------------------------------------------------------
`include "Channel.sv"
//-------------------------------------------------------------------------------------------------

module data_generator (interface r);
  parameter WIDTH = 8;
  parameter FL = 10;
  logic [WIDTH-1:0] randValue = 0;
  always
  begin
    #FL;
    randValue = {$random()} % 2;
	$display("%m: Send value=%d, Time=", randValue, $time);
    r.Send(randValue);
    void'(r.SingleRailToP1of4()); 
    #0;
  end
endmodule

module data_bucket (interface r);
  parameter WIDTH = 8;
  parameter BL = 10;
  logic [WIDTH-1:0] ReceiveValue = 0;
  always
  begin
    r.Receive(ReceiveValue);
    $display("%m: Receive value=%d, Time=", ReceiveValue, $time);
    #BL;
  end
endmodule

module full_buffer (interface left, interface right);
  parameter FL = 2;
  parameter BL = 6;
  parameter WIDTH = 8;
  parameter STAT_EN=0;  //if 1, it calculate statistical information
  logic [WIDTH-1:0] data;
  always
  begin
    left.Receive(data);
    #FL;
    // right.Send(data);
	// bug line, flip sending to receiving
	right.Receive(data);
    #BL;
  end
endmodule

module linear_pipeline;
  //instantiate interfaces
  Channel            c1();
  Channel            c2();
  Channel            c3();
  Channel            c4();
  
  data_generator     dg(c1);
  full_buffer        fb1(c1, c2);
  full_buffer        fb2(c2, c3);
  full_buffer        fb3(c3, c4);
  data_bucket        db(c4);
endmodule