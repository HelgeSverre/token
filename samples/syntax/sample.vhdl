-- VHDL Syntax Highlighting Test
-- A UART transmitter with configurable baud rate and FIFO buffer.

library IEEE;
use IEEE.std_logic_1164.all;
use IEEE.numeric_std.all;

-- ============================================================
-- Package: UART constants and types
-- ============================================================

package uart_pkg is
    -- Constants
    constant CLK_FREQ    : natural := 100_000_000;  -- 100 MHz
    constant BAUD_RATE   : natural := 115_200;
    constant DATA_BITS   : natural := 8;
    constant STOP_BITS   : natural := 1;
    constant FIFO_DEPTH  : natural := 16;

    -- Derived constants
    constant CLKS_PER_BIT : natural := CLK_FREQ / BAUD_RATE;
    constant BIT_COUNTER_MAX : natural := CLKS_PER_BIT - 1;

    -- Types
    type uart_state_t is (
        ST_IDLE,
        ST_START,
        ST_DATA,
        ST_PARITY,
        ST_STOP,
        ST_CLEANUP
    );

    type parity_t is (PARITY_NONE, PARITY_EVEN, PARITY_ODD);

    -- Configuration record
    type uart_config_t is record
        baud_rate   : natural;
        data_bits   : natural range 5 to 9;
        stop_bits   : natural range 1 to 2;
        parity      : parity_t;
        flow_ctrl   : boolean;
    end record uart_config_t;

    constant DEFAULT_CONFIG : uart_config_t := (
        baud_rate => BAUD_RATE,
        data_bits => DATA_BITS,
        stop_bits => STOP_BITS,
        parity    => PARITY_NONE,
        flow_ctrl => false
    );

    -- Function declarations
    function calc_parity(data : std_logic_vector; ptype : parity_t)
        return std_logic;
    function log2_ceil(n : natural) return natural;
end package uart_pkg;

package body uart_pkg is

    function calc_parity(data : std_logic_vector; ptype : parity_t)
        return std_logic is
        variable p : std_logic := '0';
    begin
        for i in data'range loop
            p := p xor data(i);
        end loop;
        case ptype is
            when PARITY_EVEN => return p;
            when PARITY_ODD  => return not p;
            when PARITY_NONE => return '0';
        end case;
    end function;

    function log2_ceil(n : natural) return natural is
        variable result : natural := 0;
        variable value  : natural := 1;
    begin
        while value < n loop
            result := result + 1;
            value  := value * 2;
        end loop;
        return result;
    end function;

end package body uart_pkg;

-- ============================================================
-- Entity: FIFO buffer
-- ============================================================

library IEEE;
use IEEE.std_logic_1164.all;
use IEEE.numeric_std.all;
use work.uart_pkg.all;

entity fifo is
    generic (
        DEPTH : natural := FIFO_DEPTH;
        WIDTH : natural := DATA_BITS
    );
    port (
        clk     : in  std_logic;
        rst_n   : in  std_logic;

        -- Write port
        wr_en   : in  std_logic;
        wr_data : in  std_logic_vector(WIDTH - 1 downto 0);

        -- Read port
        rd_en   : in  std_logic;
        rd_data : out std_logic_vector(WIDTH - 1 downto 0);

        -- Status
        full    : out std_logic;
        empty   : out std_logic;
        count   : out unsigned(log2_ceil(DEPTH) downto 0)
    );
end entity fifo;

architecture rtl of fifo is
    constant ADDR_WIDTH : natural := log2_ceil(DEPTH);
    type mem_t is array (0 to DEPTH - 1) of std_logic_vector(WIDTH - 1 downto 0);

    signal memory   : mem_t := (others => (others => '0'));
    signal wr_ptr   : unsigned(ADDR_WIDTH - 1 downto 0) := (others => '0');
    signal rd_ptr   : unsigned(ADDR_WIDTH - 1 downto 0) := (others => '0');
    signal cnt      : unsigned(ADDR_WIDTH downto 0) := (others => '0');
    signal full_i   : std_logic;
    signal empty_i  : std_logic;
begin
    full_i  <= '1' when cnt = DEPTH else '0';
    empty_i <= '1' when cnt = 0     else '0';
    full    <= full_i;
    empty   <= empty_i;
    count   <= cnt;

    process(clk, rst_n)
    begin
        if rst_n = '0' then
            wr_ptr <= (others => '0');
            rd_ptr <= (others => '0');
            cnt    <= (others => '0');
        elsif rising_edge(clk) then
            -- Simultaneous read and write
            if wr_en = '1' and full_i = '0' and rd_en = '1' and empty_i = '0' then
                memory(to_integer(wr_ptr)) <= wr_data;
                rd_data <= memory(to_integer(rd_ptr));
                wr_ptr <= wr_ptr + 1;
                rd_ptr <= rd_ptr + 1;
                -- Count stays the same

            elsif wr_en = '1' and full_i = '0' then
                memory(to_integer(wr_ptr)) <= wr_data;
                wr_ptr <= wr_ptr + 1;
                cnt    <= cnt + 1;

            elsif rd_en = '1' and empty_i = '0' then
                rd_data <= memory(to_integer(rd_ptr));
                rd_ptr  <= rd_ptr + 1;
                cnt     <= cnt - 1;
            end if;
        end if;
    end process;
end architecture rtl;

-- ============================================================
-- Entity: UART Transmitter
-- ============================================================

library IEEE;
use IEEE.std_logic_1164.all;
use IEEE.numeric_std.all;
use work.uart_pkg.all;

entity uart_tx is
    generic (
        CONFIG : uart_config_t := DEFAULT_CONFIG
    );
    port (
        clk      : in  std_logic;
        rst_n    : in  std_logic;

        -- Data interface
        tx_data  : in  std_logic_vector(CONFIG.data_bits - 1 downto 0);
        tx_valid : in  std_logic;
        tx_ready : out std_logic;

        -- Serial output
        tx_out   : out std_logic;

        -- Status
        tx_busy  : out std_logic
    );
end entity uart_tx;

architecture rtl of uart_tx is
    constant CLKS_PER : natural := CLK_FREQ / CONFIG.baud_rate;

    signal state      : uart_state_t := ST_IDLE;
    signal clk_count  : natural range 0 to CLKS_PER - 1 := 0;
    signal bit_index  : natural range 0 to CONFIG.data_bits - 1 := 0;
    signal shift_reg  : std_logic_vector(CONFIG.data_bits - 1 downto 0) := (others => '0');
    signal tx_done    : std_logic := '0';
    signal stop_count : natural range 0 to CONFIG.stop_bits - 1 := 0;
begin

    tx_ready <= '1' when state = ST_IDLE else '0';
    tx_busy  <= '0' when state = ST_IDLE else '1';

    -- Main state machine
    process(clk, rst_n)
    begin
        if rst_n = '0' then
            state     <= ST_IDLE;
            tx_out    <= '1';  -- Idle high
            clk_count <= 0;
            bit_index <= 0;

        elsif rising_edge(clk) then
            case state is

                when ST_IDLE =>
                    tx_out    <= '1';
                    clk_count <= 0;
                    bit_index <= 0;

                    if tx_valid = '1' then
                        shift_reg <= tx_data;
                        state     <= ST_START;
                    end if;

                when ST_START =>
                    tx_out <= '0';  -- Start bit

                    if clk_count < CLKS_PER - 1 then
                        clk_count <= clk_count + 1;
                    else
                        clk_count <= 0;
                        state     <= ST_DATA;
                    end if;

                when ST_DATA =>
                    tx_out <= shift_reg(bit_index);

                    if clk_count < CLKS_PER - 1 then
                        clk_count <= clk_count + 1;
                    else
                        clk_count <= 0;

                        if bit_index < CONFIG.data_bits - 1 then
                            bit_index <= bit_index + 1;
                        else
                            bit_index <= 0;
                            if CONFIG.parity /= PARITY_NONE then
                                state <= ST_PARITY;
                            else
                                state <= ST_STOP;
                            end if;
                        end if;
                    end if;

                when ST_PARITY =>
                    tx_out <= calc_parity(shift_reg, CONFIG.parity);

                    if clk_count < CLKS_PER - 1 then
                        clk_count <= clk_count + 1;
                    else
                        clk_count <= 0;
                        state     <= ST_STOP;
                    end if;

                when ST_STOP =>
                    tx_out <= '1';  -- Stop bit(s)

                    if clk_count < CLKS_PER - 1 then
                        clk_count <= clk_count + 1;
                    else
                        clk_count <= 0;
                        if stop_count < CONFIG.stop_bits - 1 then
                            stop_count <= stop_count + 1;
                        else
                            stop_count <= 0;
                            state      <= ST_CLEANUP;
                        end if;
                    end if;

                when ST_CLEANUP =>
                    state <= ST_IDLE;

            end case;
        end if;
    end process;

end architecture rtl;
