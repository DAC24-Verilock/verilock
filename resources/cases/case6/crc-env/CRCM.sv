`include "Channel.sv"

module CRC(interface data,
           interface count,
           interface result);
  Channel com();
  FSM fsm(com, count);
  Compute compute(com, data, result);
endmodule

module FSM(interface command,
           interface count);
  always begin
    logic [7:0] s;
    command.Send(1);
    count.Receive(s);
    while (s != 0) begin
      command.Send(2);
      s = s - 1; 
    end
    command.Send(3);
  end
endmodule

module Compute(interface command,
               interface data,
               interface result);
  always begin
    logic [7:0] state, d, a, b;
    command.Receive(state);
    if (state == 1) begin 
      b = 0; 
    end else if (state == 2) begin
      data.Receive(d);
      // CRC computation logic
    end else begin
      data.Receive(a);
      if (a == b) begin 
        result.Send(1); 
      end else begin 
        result.Send(0); 
      end
    end
  end
endmodule

module Monitor(interface notification,
               interface result);
    logic complete;
    logic r;
    always begin
        notification.Receive(complete);
        result.Receive(r);
        // omitted logic
    end
endmodule

module Top(interface data,
           interface count,
           interface notification);
  Channel result();
  Monitor monitor(notification, result);
  CRC crc(data, count, result);
endmodule