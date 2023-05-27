module Job exposing
    ( Job
    , JobHandle
    , JobPollingState(..)
    , JobResult(..)
    , asResult
    , decoder
    , handleDecoder
    , loadPendingJob
    )

import Environment
import Http
import Json.Decode as Decode


type alias JobHandle =
    { id : String }


type JobPollingState
    = WaitingForId
    | PollingId String
    | PolledId String


type alias Job =
    { status : Maybe String
    , result : Maybe String
    }


type JobResult
    = Pending
    | Failed String
    | Success
    | Unknown


jobEnumeration : Maybe String -> Decode.Decoder Job
jobEnumeration maybeStatus =
    case maybeStatus of
        Nothing ->
            Decode.succeed { status = Just "pending", result = Nothing }

        Just "pending" ->
            Decode.succeed { status = Just "pending", result = Nothing }

        Just "success" ->
            Decode.succeed { status = Just "success", result = Nothing }

        Just "failure" ->
            Decode.map2 Job
                (Decode.field "beetle:kind" (Decode.maybe Decode.string))
                (Decode.field "beetle:content" (Decode.maybe Decode.string))

        _ ->
            Decode.succeed { status = Just "unknown", result = Nothing }


handleDecoder : Decode.Decoder JobHandle
handleDecoder =
    Decode.map JobHandle (Decode.field "id" Decode.string)


decoder : Decode.Decoder Job
decoder =
    Decode.field "beetle:kind" (Decode.maybe Decode.string)
        |> Decode.andThen jobEnumeration


asResult : Job -> JobResult
asResult job =
    case ( job.status, job.result ) of
        ( Just "pending", _ ) ->
            Pending

        ( Nothing, _ ) ->
            Pending

        ( Just "success", _ ) ->
            Success

        ( Just "failure", Just reason ) ->
            Failed reason

        _ ->
            Unknown


loadPendingJob : Environment.Environment -> (Result Http.Error Job -> a) -> JobHandle -> Cmd a
loadPendingJob env message handle =
    let
        url =
            Environment.apiRoute env "jobs" ++ "?id=" ++ handle.id
    in
    Http.get { url = url, expect = Http.expectJson message decoder }
