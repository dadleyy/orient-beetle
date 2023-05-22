module Job exposing (Job, JobResult(..), asResult, decoder)

import Json.Decode as Decode


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
