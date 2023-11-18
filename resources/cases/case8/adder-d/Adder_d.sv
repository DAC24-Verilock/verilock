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

module Adder (interface A, interface B, interface SUM);
  parameter WIDTH = 8; 
  logic [WIDTH-1:0] a = 0, b=0, sum=0;
  always
  begin
	fork
		A.Receive(a);
		B.Receive(b);
	join
	sum = a + b ;
	// SUM.Send(sum);
	// bug line, flip sending to receiving
	SUM.Receive(sum);
  end
endmodule

module linear_pipeline;
  //instantiate interfaces
  Channel            c1();
  Channel            c2();
  Channel            c3();
  
  data_generator     dg1(c1);
  data_generator     dg2(c2);
  Adder              ad(c1, c2, c3);
  data_bucket        db(c3);
endmodule