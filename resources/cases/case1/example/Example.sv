`include "Channel.sv"

module Controller(interface A,
                  interface B, 
                  interface C);
  logic [7:0] response;
  always begin
    fork
      A.Send(0);
      B.Send(1);
    join
    C.Receive(response);
  end
endmodule

module Worker(interface A,
              interface B,
              interface C);
  logic [7:0] flag;
  always begin
    fork
      A.Receive(flag);
      B.Receive(flag);
    join
    if (flag > 0) begin
        C.Send(42); 
    end else begin
        C.Send(0);
    end
  end
endmodule

module Top;
  Channel A();
  Channel B();
  Channel C();
  Controller c(A, B, C);
  Worker w(A, B, C);
endmodule