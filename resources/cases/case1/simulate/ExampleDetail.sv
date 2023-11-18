`include "Channel.sv"

module Controller(interface A, 
                  interface B, 
                  interface C);
  logic [7:0] response;
  always begin
    $display("Before sending, time[%0t]", $time);
    fork
      A.Send(0);
      B.Send(1);
    join
    $display("After sending, time[%0t]", $time);
    #1;
    C.Receive(response);
    $display("Response[%0d]", response);
  end
endmodule

module Worker(interface A,
              interface B,
              interface C);
  logic [7:0] flag;
  always begin
    $display("Before receiving, time[%0t]", $time);
    fork
      A.Receive(flag);
      B.Receive(flag);
    join
    $display("After receiving, time[%0t]", $time);
    #1;
    if (!flag) begin C.Send(42); end
    $display("Sending response, time[%0t]", $time);
  end
endmodule

module Top;
  Channel A(), B(), C();
  Controller c(A, B, C);
  Worker w(A, B, C);
endmodule