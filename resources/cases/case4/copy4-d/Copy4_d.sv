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

module copy4 (interface in_I, 
			  interface out0_I, 
			  interface out1_I, 
			  interface out2_I, 
			  interface out3_I );
  parameter WIDTH = 8;
  parameter FL=2, BL=8;
  logic [WIDTH-1:0] data;
  always
  begin
    in_I.Receive(data);
    #FL;
    wait (out0_I.status != idle  && out1_I.status != idle && out2_I.status !=idle && out3_I.status != idle );
    fork
      out0_I.Send(data);
      out1_I.Send(data);
      out2_I.Send(data);
      out3_I.Send(data);
    join 
    #BL;
  end
endmodule

module Top;
  Channel in_I();
  Channel out0_I();
  Channel out1_I();
  Channel out2_I();
  Channel out3_I();
  data_generator dg(in_I);
  copy4 c4(in_I, out0_I, out1_I, out2_I, out3_I);
  data_bucket db0(out0_I);
  data_bucket db1(out1_I);
//   data_bucket db2(out2_I);
// bug line, misconnection
  data_bucket db2(out1_I);
  data_bucket db3(out3_I);
endmodule