use core::fmt::{Debug, Formatter};
use embassy_rp::peripherals::UART0;
use embassy_rp::uart;
use embassy_rp::uart::BufferedUart;
use embassy_sync::blocking_mutex::CriticalSectionMutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Sender;
use embassy_time::{Duration, WithTimeout};
use embedded_io_async::{Read, Write};
use static_cell::StaticCell;
use crate::{elm_commands, errors, ElmUart, ToMainEvents, Irqs, INCOMING_EVENT_CHANNEL};
use crate::errors::{ToRustAGaugeError, ToRustAGaugeErrorSeverity, ToRustAGaugeErrorWithSeverity};

const LOCAL_RX_BUFFER_LEN: usize = 256;
const UART_TIMEOUT: Duration = Duration::from_millis(100u64);

#[embassy_executor::task]
pub async fn elm_uart_task(r: ElmUart){
    let sender: Sender<CriticalSectionRawMutex, ToMainEvents, 10> = INCOMING_EVENT_CHANNEL.sender();
    let (tx_pin, rx_pin, uart) = (r.tx_pin, r.rx_pin, r.uart0);
    static TX_BUF: StaticCell<[u8; 16]> = StaticCell::new();
    let tx_buf = &mut TX_BUF.init([0; 16])[..];
    static RX_BUF: StaticCell<[u8; 16]> = StaticCell::new();
    let rx_buf = &mut RX_BUF.init([0; 16])[..];

    let uart_config = uart::Config::default();

    let mut uart = BufferedUart::new(uart, Irqs, tx_pin, rx_pin, tx_buf, rx_buf, uart_config);
    
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::ELM_RESET.into()
    ).await, sender, ToRustAGaugeErrorSeverity::MaybeRecoverable).await;
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::DISABLE_ECHO.into()
    ).await, sender, ToRustAGaugeErrorSeverity::CompleteFailure).await;
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::ENABLE_HEADERS.into()
    ).await, sender, ToRustAGaugeErrorSeverity::CompleteFailure).await;
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::SET_PROTOCOL_5.into()
    ).await, sender, ToRustAGaugeErrorSeverity::CompleteFailure).await;
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::SET_TIMEOUT_64.into()
    ).await, sender, ToRustAGaugeErrorSeverity::EntirelyRecoverable).await;
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::DISABLE_SPACES.into()
    ).await, sender, ToRustAGaugeErrorSeverity::MaybeRecoverable).await;
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::DISABLE_MEMORY.into()
    ).await, sender, ToRustAGaugeErrorSeverity::MaybeRecoverable).await;
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::ENABLE_AUTO_TIMINGS_1.into()
    ).await, sender, ToRustAGaugeErrorSeverity::EntirelyRecoverable).await;
    result_unpacker(uart_write_read(
        &mut uart, elm_commands::SET_CUSTOM_HEADERS.into()
    ).await, sender, ToRustAGaugeErrorSeverity::CompleteFailure).await;
    
    
    

}
async fn result_unpacker<'a, T, E>(result: Result<T, E>, 
                         sender: Sender<'a, CriticalSectionRawMutex, ToMainEvents, 10>,
                         error_severity: ToRustAGaugeErrorSeverity
) -> Option<T> 
where E: Into<ToRustAGaugeError>
{
    match result{
        Ok(v) => {
            Some(v)
        }
        Err(e) => {
            let error = ToRustAGaugeErrorWithSeverity::from_with_severity(e, error_severity);
            sender.send(ToMainEvents::ElmError(error)).await;
            None
        }
    }
}

struct SizedUartBuffer<'a>{
    buffer: &'a[u8; LOCAL_RX_BUFFER_LEN],
    end: usize,
}

impl Debug for SizedUartBuffer<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let str = String::from_utf8_lossy(&self.buffer[..self.end]);
        write!(f, "{}", str)
    }
}


/// reads from Uart until it finds the delimiter. Could read past the delimiter, but in this use case
/// there are no unprompted writes to uart 
/// 
/// # Arguments 
/// 
/// * `uart`: 
/// * `delimiter_char`: 
/// 
/// returns: Result<SizedUartBuffer, ToRustAGaugeError> 
/// 
/// # Examples 
/// 
/// ```
/// 
/// ```
async fn uart_read_until_char<'a>(uart: &mut BufferedUart<'a, UART0>,
                                  delimiter: u8
) -> Result<SizedUartBuffer<'a>, ToRustAGaugeError>{

    let mut index: usize = 0;
    
    let mut local_rx_buffer: [u8; LOCAL_RX_BUFFER_LEN] = [0u8; LOCAL_RX_BUFFER_LEN];
    
    let mut is_delimiter_found: bool = false;
    
    while !is_delimiter_found && index < LOCAL_RX_BUFFER_LEN {
        
        let temp_slice = &mut local_rx_buffer[index..];
        
        match uart.read(temp_slice).with_timeout(UART_TIMEOUT).await{
            Ok(Ok(len)) => { // timeout OK( UartRead OK( length read ) ) 
                if temp_slice[..len].contains(&delimiter){
                    is_delimiter_found = true;
                }
                index = index + len;
            }
            Ok(Err(e)) => { // timeout OK( UartRead Err( UartError ) ) 
                return Err(ToRustAGaugeError::UartError(e));
            }
            Err(e) => {
                return Err(ToRustAGaugeError::UartTimeoutError(e));
            }
        }
    }
    
    if is_delimiter_found {
        return Ok(SizedUartBuffer{
            buffer: &local_rx_buffer,
            end: index,
        })
    }
    
    Err(ToRustAGaugeError::UartBufferOverflowError())
}

async fn uart_write_read<'a>(uart: &mut BufferedUart<'a, UART0>, message: &[u8]
) -> Result<SizedUartBuffer<'a>, ToRustAGaugeError>{
    uart.write(message).with_timeout(UART_TIMEOUT).await??;
    let result = uart_read_until_char(uart, b'>').await?;
    log::info!("{:?}", result);
    Ok(result)
    
}
