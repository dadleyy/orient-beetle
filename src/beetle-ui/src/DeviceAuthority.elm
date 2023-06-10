module DeviceAuthority exposing (DeviceAuthorityModel, DeviceAuthorityResponse, fetchDeviceAuthority)

import Environment
import Http
import Json.Decode as D


type alias DeviceAuthorityModel =
    { kind : String
    }


type alias DeviceAuthorityResponse =
    { deviceId : String
    , authorityModel : DeviceAuthorityModel
    }


authorityModelDecoder : D.Decoder DeviceAuthorityModel
authorityModelDecoder =
    D.map DeviceAuthorityModel
        (D.field "beetle:kind" D.string)


authorityDecoder : D.Decoder DeviceAuthorityResponse
authorityDecoder =
    D.map2 DeviceAuthorityResponse
        (D.field "device_id" D.string)
        (D.field "authority_model" authorityModelDecoder)


fetchDeviceAuthority : Environment.Environment -> String -> (Result Http.Error DeviceAuthorityResponse -> a) -> Cmd a
fetchDeviceAuthority env id messageKind =
    Http.get
        { url = Environment.apiRoute env ("device-authority?id=" ++ id)
        , expect = Http.expectJson messageKind authorityDecoder
        }
