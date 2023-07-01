module DeviceSchedule exposing (fetchDeviceSchedule)

import Environment
import Http


fetchDeviceSchedule : Environment.Environment -> String -> (Result Http.Error () -> a) -> Cmd a
fetchDeviceSchedule env id messageKind =
    Http.get
        { url = Environment.apiRoute env ("device-schedules?device_id=" ++ id)
        , expect = Http.expectWhatever messageKind
        }
